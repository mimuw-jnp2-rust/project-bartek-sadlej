use serde::{Deserialize, Serialize};

type Cookie = String;

#[derive(Debug)]
pub struct ChatDatabase {

}

impl ChatDatabase {
    #[allow(dead_code)]
    pub fn authenticate_user(name : String, password : String) -> Cookie {
        "".into()
    }
}

// for now cookie it is always empty, but will be usefull later to introduce remembering the state  
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthenticationToken {
    user_name : String,
    cookie : Cookie,
}
