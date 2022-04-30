use serde::{Serialize, Deserialize};

use crate::database::AuthenticationToken;
use crate::channel::ChannelInfo;

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    FromSerever { content : ServerMessage},
    FromUser { token : Option<AuthenticationToken>, message : UserMessage},
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    ChannelInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UserMessage {
    UserJoin { name : String },
    TextMessage { content : String }
}