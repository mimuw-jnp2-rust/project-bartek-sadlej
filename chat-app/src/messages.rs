use serde::{Serialize, Deserialize};

use crate::{database::AuthenticationToken, channel::ChannelInfo};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {

    // When user connects to server, it sends them channel info
    ChannelsInfo { channels : Vec<ChannelInfo> }, 

    // response to Connect message with optional error descritpion
    ConnectResponse { token : AuthenticationToken, error : Option<ChatError> },

    // All users in channel gets informed about new user
    UserJoined { name : String },

    // text messages send in channel
    TextMessage { content : String }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UserMessage {

    // user wants to connect, in response if gets ConnectAccepted
    Connect { name : String, password : String },

    // user sends when they choosed channel to join  
    Join { token : AuthenticationToken, channel_name : String },

    // text messages send in channel
    TextMessage { token : AuthenticationToken, content : String }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ChatError {
    InvalidPassword,
    NameUsed,
}