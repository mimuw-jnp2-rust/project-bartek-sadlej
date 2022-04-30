use serde::{Serialize, Deserialize};

use crate::database::AuthenticationToken;
use crate::channel::ChannelInfo;

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    ServerMessage,
    UserMessage,
}

#[derive(Serialize, Deserialize, Debug)]

pub enum ServerMessage {
    ChannelInfo,

}
