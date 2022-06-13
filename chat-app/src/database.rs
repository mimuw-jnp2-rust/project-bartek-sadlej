use std::{net::SocketAddr, collections::HashMap};
use std::sync::RwLock;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

use serde::{Deserialize, Serialize};
use tokio_postgres::{Client, Row};

use crate::utils::{ChatError, calculate_hash};
use anyhow::{Context, Result};

type Cookie = String;

#[derive(Debug)]
pub struct ChatDatabase {
    client: Client,
    tokens: RwLock<HashMap<SocketAddr, AuthenticationToken>>,
}

// const database = new Sequelize("bd", "bs429589", "iks", {
//     host: "localhost",
//     port: "11212",
//     dialect: "postgres",
// });

impl ChatDatabase {
    pub fn new(client: Client) -> ChatDatabase {
        ChatDatabase { client, tokens : RwLock::new(HashMap::new()) }
    }

    #[allow(dead_code)]
    pub async fn authenticate_user(
        &self,
        name: String,
        password: String,
        addr: SocketAddr,
    ) -> Result<AuthenticationToken, ChatError> {
        let password_hash = calculate_hash(&password) as i64;
        match self.client.query("SELECT * FROM users WHERE name = ($1) AND password = ($2)", &[&name, &password_hash]).await {
            Ok(rows) => {
                if rows.len() == 0 {
                    return Err(ChatError::InvalidPassword);
                }
            },
            Err(e) => {
                tracing::debug!("{:?}", e);
                return Err(ChatError::DatabaseError);
            },
        }
    
        let token = AuthenticationToken {
            user_name: name,
            cookie: thread_rng()
                .sample_iter(&Alphanumeric)
                .take(32)
                .map(char::from)
                .collect(),
        };
        self.tokens.write().unwrap().insert(addr, token.clone());
        Ok(token)
    }

    pub fn authorize_connection(&self, _token: &AuthenticationToken, _addrr: &SocketAddr) -> bool {
        true
    }

    pub async fn get_channels_names(&self) -> Result<Vec<String>> {
        let results = self.client.query("SELECT name FROM channels", &[]).await.context("Error selecting channel names from database!")?;
        let channel_names : Vec<String> =  results.into_iter().map(|row| ChannelData::from(row).name).collect();
        Ok(channel_names)
    }

    pub async fn create_channel(&self, name: &str) ->Result<()> {
        self.client.execute("INSERT INTO channels (name) VALUES ($1)", &[&name]).await.context("Error inserting new channel to db!")?;
        Ok(())
    } 
}

// for now cookie it is always empty, but will be usefull later to introduce remembering the state
#[derive(Serialize, Deserialize, Debug, Clone)]
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
