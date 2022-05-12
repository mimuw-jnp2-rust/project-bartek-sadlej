use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use crate::messages::{ServerMessage, UserMessage};

pub async fn get_next_server_message(lines: &mut Framed<TcpStream, LinesCodec>) -> Option<Result<ServerMessage, serde_json::Error>> {
    let line = match lines.next().await {
        Some(Ok(line)) => line,
        x => {
            tracing::error!("{:?}", x);
            return None;
        }
    };
    let decoded_msg: Result<ServerMessage, serde_json::Error> = serde_json::from_str(&line);
    tracing::debug!("Received new message: {:?}", decoded_msg);

    match decoded_msg {
        Ok(msg) => Some(Ok(msg)),
        _ => None,
    }
}

pub async fn get_next_user_message(lines: &mut Framed<TcpStream, LinesCodec>) -> Option<Result<UserMessage, serde_json::Error>> {
    let line = match lines.next().await {
        Some(Ok(line)) => line,
        x => {
            tracing::error!("Received message: {:?}", x);
            return None;
        }
    };
    let decoded_msg: Result<UserMessage, serde_json::Error> = serde_json::from_str(&line);
    tracing::debug!("Received new message: {:?}", decoded_msg);

    match decoded_msg {
        Ok(msg) => Some(Ok(msg)),
        _ => None,
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ChatError {
    InvalidPassword,
    NameUsed,
}