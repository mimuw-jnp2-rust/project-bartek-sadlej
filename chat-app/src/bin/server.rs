use std::env;
use std::error::Error;
use std::net::SocketAddr;


use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::messages::{ServerMessage, UserMessage};
use chat_app::channel::{Channel, ChannelInfo};
use chat_app::common::{get_next_server_message, get_next_user_message};

use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LinesCodec};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env::set_var("RUST_LOG", "debug");

    // --- CONFIGURE LOGGING ---
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("chat=info".parse()?))
    .with_span_events(FmtSpan::FULL)
    .init();
    // --- ----------------- ---

    // --- CONFIGURE CHANNELS ---
    let mut channels : Vec<Channel> = Vec::new();
    for channel_name in env::args() {
        channels.push(Channel::new(channel_name).await);
    }
    let channels_info : Vec<ChannelInfo> = channels.iter().skip(1).map(|channel| channel.get_channel_info()).collect();
    tracing::info!("Created channels: {:?}", channels_info);
    // --- ------------------ ---

    // --- START SERVER ---
    let server_address = SocketAddr::new(SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT);
    let listener = TcpListener::bind(server_address)
    .await
    .expect("Error starting server!");
    tracing::info!("Server running on {}", server_address);
    // --- ------------ ---

    // --- ACCEPT LOOP ---
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            tracing::info!("accepted connection {}", addr);
            if let Err(e) = handle_new_user(stream, addr).await {
                tracing::info!("an error occurred; error = {:?}", e);
            }
        });
    }
    // --- ----------- ---
}

async fn handle_new_user(stream: TcpStream,
    addr: SocketAddr)  -> Result<(), Box<dyn Error>> {
        let mut lines = Framed::new(stream, LinesCodec::new());

        let username = match get_next_user_message(&mut lines).await {
            Some(Ok(UserMessage::Connect{ name , password})) => {
                name
            },
            _ => {
                tracing::error!(
                    "Failed to get joining message. Client {} disconnected.",
                    addr
                );
                return Ok(());
            }
        };

    Ok(())
}
