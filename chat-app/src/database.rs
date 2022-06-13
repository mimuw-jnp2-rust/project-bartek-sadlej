use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio_postgres::{Client, Row};

use crate::utils::ChatError;
use anyhow::{Context, Result};

type Cookie = String;

#[derive(Debug)]
pub struct ChatDatabase {
    client: Client,
}

// const database = new Sequelize("bd", "bs429589", "iks", {
//     host: "localhost",
//     port: "11212",
//     dialect: "postgres",
// });

impl ChatDatabase {
    pub fn new(client: Client) -> ChatDatabase {
        ChatDatabase { client }
    }

    #[allow(dead_code)]
    pub fn authenticate_user(
        &self,
        name: String,
        _password: String,
        _addr: SocketAddr,
    ) -> Result<AuthenticationToken, ChatError> {
        Ok(AuthenticationToken {
            user_name: name,
            cookie: "".into(),
        })
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
