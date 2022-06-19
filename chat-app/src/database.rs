use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use tokio_postgres::{Client, Row};

use crate::utils::{self, calculate_hash, ChatError};
use anyhow::{Context, Result};

type Cookie = String;

#[derive(Debug)]
pub struct ChatDatabase {
    client: Client,
    tokens: RwLock<HashMap<String, String>>,
}

impl ChatDatabase {
    pub fn new(client: Client) -> ChatDatabase {
        ChatDatabase {
            client,
            tokens: RwLock::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    pub async fn authenticate_user(
        &self,
        name: &str,
        password: &str,
    ) -> Result<AuthenticationToken, ChatError> {
        let password_hash = calculate_hash(&password) as i64;
        match self
            .client
            .query(
                "SELECT * FROM users WHERE name = ($1) AND password = ($2)",
                &[&name, &password_hash],
            )
            .await
        {
            Ok(rows) => {
                if rows.is_empty() {
                    return Err(ChatError::InvalidPassword);
                }
            }
            Err(e) => {
                tracing::debug!("{:?}", e);
                return Err(utils::ChatError::DatabaseError(e));
            }
        }

        let token = AuthenticationToken {
            user_name: name.to_string(),
            cookie: thread_rng()
                .sample_iter(&Alphanumeric)
                .take(32)
                .map(char::from)
                .collect(),
        };
        self.tokens
            .write()
            .unwrap()
            .insert(name.to_string(), token.cookie.clone());
        Ok(token)
    }

    pub fn authorize_connection(&self, token: &AuthenticationToken) -> bool {
        match self.tokens.read().unwrap().get(&token.user_name) {
            Some(cookie) => token.cookie.eq(cookie),
            None => false,
        }
    }

    pub async fn get_channels_names(&self) -> Result<Vec<String>> {
        let results = self
            .client
            .query("SELECT name FROM channels", &[])
            .await
            .context("Error selecting channel names from database!")?;
        let results = results.into_iter().map(ChannelData::from);
        let channel_names: Vec<String> = results
            .into_iter()
            .map(|channel_data| channel_data.name)
            .collect();
        Ok(channel_names)
    }

    pub async fn create_channel(&self, name: &str) -> Result<()> {
        self.client
            .execute("INSERT INTO channels (name) VALUES ($1)", &[&name])
            .await
            .context("Error inserting new channel to database!")?;
        Ok(())
    }

    pub async fn create_user(&self, name: &str, password: &str) -> Result<()> {
        let password_hash = calculate_hash(&password) as i64;
        self.client
            .execute(
                "INSERT INTO users (name, password) VALUES ($1, $2)",
                &[&name, &password_hash],
            )
            .await
            .context("Error inserting new user to database!")?;
        Ok(())
    }

    pub async fn get_unseed_messages(
        &self,
        channel_name: &str,
        user_name: &str,
    ) -> Result<Vec<(String, String)>, ChatError> {
        let last_seen_message_id: Result<Row, tokio_postgres::Error> = self
            .client
            .query_one(
                "SELECT message_id FROM history WHERE user_name = ($1) AND channel_name = ($2)",
                &[&user_name, &channel_name],
            )
            .await;
        let last_seen_message_id: i32 = last_seen_message_id.map_or(-1, |row| row.get(0));
        let results = self
            .client
            .query(
                "SELECT user_name, content FROM messages WHERE channel_name = ($1) AND id > ($2)",
                &[&channel_name, &last_seen_message_id],
            )
            .await?;
        Ok(results
            .into_iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect())
    }

    pub async fn save_message(
        &self,
        channel_name: &str,
        user_name: &str,
        message: &str,
    ) -> Result<(), ChatError> {
        self.client
            .execute(
                "INSERT INTO messages (channel_name, user_name, content) VALUES ($1, $2, $3)",
                &[&channel_name, &user_name, &message],
            )
            .await?;
        Ok(())
    }

    pub async fn save_history(&self, channel_name: &str, user_name: &str) -> Result<(), ChatError> {
        self.client
            .execute(
                "
            INSERT INTO history (user_name, channel_name, message_id)
            VALUES (
                ($1),
                ($2),
                (SELECT coalesce(MAX(id),-1) as message_id FROM messages 
                WHERE channel_name = ($2)))
            ON CONFLICT (user_name, channel_name) DO UPDATE
                SET message_id = excluded.message_id;",
                &[&user_name, &channel_name],
            )
            .await?;
        Ok(())
    }
}

// for now cookie it is always empty, but will be usefull later to introduce remembering the state
#[derive(Serialize, Deserialize, Debug, Clone, Hash)]
pub struct AuthenticationToken {
    pub user_name: String,
    cookie: Cookie,
}

#[derive(Debug)]
struct ChannelData {
    pub name: String,
}

impl From<Row> for ChannelData {
    fn from(row: Row) -> Self {
        Self {
            name: row.get("name"),
        }
    }
}
