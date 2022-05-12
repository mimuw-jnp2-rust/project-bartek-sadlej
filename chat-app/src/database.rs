use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::utils::ChatError;

type Cookie = String;

#[derive(Debug)]
pub struct ChatDatabase {}

impl ChatDatabase {
    pub fn new() -> Self {
        ChatDatabase {}
    }

    #[allow(dead_code)]
    pub fn authenticate_user(
        &self,
        name: String,
        password: String,
        addr: SocketAddr,
    ) -> Result<AuthenticationToken, ChatError> {
        Ok(AuthenticationToken {
            user_name: name,
            cookie: "".into(),
        })
    }

    pub fn authorize_connection(token: AuthenticationToken, addr: SocketAddr) -> bool {
        true
    }
}

// for now cookie it is always empty, but will be usefull later to introduce remembering the state
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthenticationToken {
    user_name: String,
    cookie: Cookie,
}
