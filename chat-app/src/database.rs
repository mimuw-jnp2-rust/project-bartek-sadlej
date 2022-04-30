use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug)]
pub struct AuthenticationToken {
    UserName : String,
    cookie : String,
}
