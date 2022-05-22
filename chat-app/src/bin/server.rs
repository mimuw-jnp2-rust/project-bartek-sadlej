use std::env;
use std::error::Error;
// use std::error::Error;
use std::net::SocketAddr;
use std::sync::{Arc};


use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::database::{ChatDatabase, AuthenticationToken};
use chat_app::messages::{ServerMessage, UserMessage};
use chat_app::channel::{Channel, ChannelInfo};
use chat_app::utils::get_next_user_message;

use futures::SinkExt;
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

    // --- DATABSE ---
    let chat_db = Arc::new(ChatDatabase::new());
    // --- ------- ---

    // --- CONFIGURE CHANNELS ---
    let mut channels_infos : Vec<ChannelInfo> = Vec::new();
    for channel_name in env::args() {
        let chat_db = Arc::clone(&chat_db);
        let new_channel= Arc::new(Channel::new(channel_name, chat_db).await);
        channels_infos.push(new_channel.get_channel_info());

        tokio::spawn(async move {
            if let Err(e) = new_channel.listen().await {
                tracing::info!("an error occurred; error = {:?}", e);
            }
        });
    }
    tracing::info!("Created channels: {:?}", channels_infos);
    let channels_info_message = Arc::new(ServerMessage::ChannelsInfo { channels: channels_infos });
    // --- ------------------ ---

    // --- START SERVER ---
    let server_address = SocketAddr::new(SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT);
    let listener = TcpListener::bind(server_address)
    .await
    .expect("[MAIN_SERVER] Error starting server!");
    tracing::info!("[MAIN_SERVER] Server running on {}", server_address);
    // --- ------------ ---

    // --- ACCEPT LOOP ---
    loop {
        let (stream, addr) = listener.accept().await.unwrap();

        let chat_db = Arc::clone(&chat_db);
        let channels_info_message = Arc::clone(&channels_info_message);
        
        tokio::spawn(async move {
            tracing::info!("[MAIN_SERVER] accepted connection {}", addr);
            if let Err(e) = handle_new_user(stream, addr, chat_db, channels_info_message).await {
                tracing::info!("[MAIN_SERVER] an error occurred; error = {:?}", e);
            }
        });
    }
    // --- ----------- ---
}

// Authenticates user and send them channels info
async fn handle_new_user(stream: TcpStream,
    addr: SocketAddr, chat_db : Arc<ChatDatabase>, channels_info_message : Arc<ServerMessage>)  -> Result<(), Box<dyn Error>> {
        
        let mut lines = Framed::new(stream, LinesCodec::new());
        
        match get_next_user_message(&mut lines).await {
            Some(Ok(UserMessage::Connect{ name , password})) => {
                if let Ok(token) = chat_db.authenticate_user(name, password, addr) {
                    if let Ok(encoded_message) = serde_json::to_string(&ServerMessage::ConnectResponse { token : Some(token), error: None }) {
                        lines.send(encoded_message).await?
                    }
                    if let Ok(encoded_message) = serde_json::to_string(channels_info_message.as_ref().into()) {
                        lines.send(encoded_message).await?
                    }
                }
                // TODO!
                // else
                // {
                //     if let Ok(encoded_message) = serde_json::to_string(&ServerMessage::ConnectResponse { token : None, error: "" }) {
                //         lines.send(encoded_message).await?
                //     }
                // }
            },
            _ => {
                tracing::error!(
                    "[MAIN_SERVER] Failed to get connect message. Client {} disconnected.",
                    addr
                );
            }
        };

    Ok(())
}
