use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::utils::ChatError;

type Cookie = String;

#[derive(Debug, Default)]
pub struct ChatDatabase {}

impl ChatDatabase {
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
}

// for now cookie it is always empty, but will be usefull later to introduce remembering the state
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthenticationToken {
    pub user_name: String,
    cookie: Cookie,
}
