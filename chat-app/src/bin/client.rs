use std::{env, error::Error};
use std::net::SocketAddr;
use std::sync::Arc;

use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::messages::{UserMessage, ServerMessage};
use chat_app::utils::get_next_server_message;
use futures::SinkExt;
use tokio_stream::StreamExt;
use tokio::net::{TcpListener, TcpStream, TcpSocket};
use tokio_util::codec::{Framed, LinesCodec};

use async_std::io;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env::set_var("RUST_LOG", "debug");

    // --- CONFIGURE LOGGING ---
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("chat=info".parse()?))
    .with_span_events(FmtSpan::FULL)
    .init();

    let user_name = env::args().nth(1).expect("usage: name password");
    let password = env::args().nth(2).expect("usage: name password");
        
    let stdin = io::stdin();
    
    loop {
        
        {
            let mut server_lines = connect_to_server().await;
            if let Ok(connect_msg) = serde_json::to_string(&UserMessage::Connect{ name : user_name.clone(), password : password.clone()}) {
                server_lines.send(&connect_msg).await?;
            }
            if let Some(Ok(ServerMessage::ChannelsInfo { channels: channels_infos })) = get_next_server_message(&mut server_lines).await {
                println!("Choose channel");
                for (idx, channel_info) in channels_infos.iter().enumerate() {
                    println!("[{}] {}", idx, channel_info.name);
                }
            }
        }
    }

    Ok(())
}

async fn connect_to_server() -> Framed<TcpStream, LinesCodec> {
    let server_address = SocketAddr::new(SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT);
    let socket = TcpSocket::new_v4().expect("Error opening socket for connection to server");
    let stream = socket.connect(server_address)
        .await
        .expect("Error connecting to server!");

    tracing::info!("Successfully connected to server {}", server_address);

    Framed::new(stream, LinesCodec::new())
}