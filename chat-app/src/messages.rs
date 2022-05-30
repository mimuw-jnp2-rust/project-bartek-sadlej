use serde::{Deserialize, Serialize};

use crate::{channel::ChannelInfo, database::AuthenticationToken};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    // When user connects to server, it sends them channel info
    ChannelsInfo {
        channels: Vec<ChannelInfo>,
    },

    // response to Connect message with optional error descritpion
    ConnectResponse {
        token: Option<AuthenticationToken>,
        error: Option<String>,
    },

    // text messages send in channel
    TextMessage {
        content: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum UserMessage {
    // user wants to connect, in response if gets ConnectAccepted
    Connect {
        name: String,
        password: String,
    },

    // user sends when they want to join channel
    Join {
        token: AuthenticationToken,
    },

    // text messages send in channel
    TextMessage {
        token: AuthenticationToken,
        content: String,
    },
}
