use std::fmt::Result;

use async_std::net::TcpListener;
use std::env;

use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::messages::Message;
use chat_app::channel::Channel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("chat=info".parse()?))
    .with_span_events(FmtSpan::FULL)
    .init();

    let channels : Vec<Channel> = env::args().map(|&channel_name| Channel::new(channel_name)).collect();
    // let channels_info = channels.map

    let server_address = (SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT);
    let listener = TcpListener::bind(server_address)
    .await
    .expect("Error starting server!");

    tracing::info!("Server running on {}", server_address);
    
    Ok(())
}