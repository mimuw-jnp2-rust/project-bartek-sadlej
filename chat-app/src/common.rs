use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, LinesCodec};
use std::error::Error;

use crate::messages::Message;

pub async fn get_next_message(lines: &mut Framed<TcpStream, LinesCodec>) -> Option<Result<Message, serde_json::Error>> {
    let line = match lines.next().await {
        Some(Ok(line)) => line,
        x => {
            tracing::error!("{:?}", x);
            return None;
        }
    };
    tracing::error!("{}", line);
    let decoded_msg: Result<Message, serde_json::Error> = serde_json::from_str(&line);

    match decoded_msg {
        Ok(msg) => Some(Ok(msg)),
        _ => None,
    }
}