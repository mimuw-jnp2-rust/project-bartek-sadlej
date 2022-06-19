use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use futures::SinkExt;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};

use anyhow::Result;
use thiserror::Error;

use crate::messages::{ServerMessage, UserMessage};

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub async fn get_next_server_message(
    lines: &mut Framed<TcpStream, LinesCodec>,
) -> Option<Result<ServerMessage>> {
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

pub async fn get_next_user_message(
    lines: &mut Framed<TcpStream, LinesCodec>,
) -> Option<Result<UserMessage>> {
    let line = match lines.next().await {
        Some(Ok(line)) => line,
        _ => return None, // disconnect
    };
    let decoded_msg: Result<UserMessage, serde_json::Error> = serde_json::from_str(&line);
    tracing::debug!("Received new message: {:?}", decoded_msg);

    match decoded_msg {
        Ok(msg) => Some(Ok(msg)),
        _ => None,
    }
}

pub async fn send_to<T: Serialize>(
    lines: &mut Framed<TcpStream, LinesCodec>,
    message: T,
) -> Result<()> {
    let msg = serde_json::to_string(&message)?;
    lines.send(&msg).await?;
    Ok(())
}

#[derive(Error, Debug)]
pub enum ChatError {
    #[error("Received invalid message")]
    InvalidMessage,
    #[error("Invalid password")]
    InvalidPassword,
    #[error("Name already used")]
    NameUsed,
    #[error("Unauthorized connection")]
    UnauthenticatedConnection,
    #[error("Runtime error")]
    RuntimeError,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error("Database error")]
    DatabaseError(#[from] tokio_postgres::Error),
}
