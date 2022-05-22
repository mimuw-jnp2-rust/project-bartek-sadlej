use std::net::SocketAddr;
use std::{env, error::Error};

use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::database::AuthenticationToken;
use chat_app::messages::{ServerMessage, UserMessage};
use chat_app::utils::get_next_server_message;
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio_util::codec::{Framed, LinesCodec};
use tokio::sync::mpsc;

use async_std::io::{self, ReadExt};

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
    let mut token: Option<AuthenticationToken> = None;
    let stdin = io::stdin();

    let mut ctrlc_channel = ctrl_channel()?;


    loop {
        let mut server_lines = connect_to_server().await;
        if let Ok(connect_msg) = serde_json::to_string(&UserMessage::Connect {
            name: user_name.clone(),
            password: password.clone(),
        }) {
            server_lines.send(&connect_msg).await?;
        }
        if let Some(Ok(ServerMessage::ConnectResponse {
            token: new_token,
            error: auth_error,
        })) = get_next_server_message(&mut server_lines).await
        {
            if let Some(error_msg) = auth_error {
                println!("{}", error_msg);
                continue;
            }
            token = new_token;
        }

        let mut channel_addr : SocketAddr;
        if let Some(Ok(ServerMessage::ChannelsInfo {
            channels: channels_infos,
        })) = get_next_server_message(&mut server_lines).await
        {
            println!("Choose channel");
            for (idx, channel_info) in channels_infos.iter().enumerate() {
                println!("[{}] {}", idx, channel_info.name);
            }

            loop {
                println!("enter number in [0 ... {}]", channels_infos.len() - 1);
                let mut line = String::new();
                stdin.read_line(&mut line).await?;
                let channel_nr = line.trim().parse::<usize>()?;
                if channel_nr < channels_infos.len() {
                    channel_addr = channels_infos[channel_nr].address;
                    break;
                }
            }
        }
        else
        {
            tracing::info!("Error receiving channels info from server");
            continue;
        }

        let channel_socket = TcpSocket::new_v4().expect("Error opening socket for connection to channel");
        let stream = channel_socket
            .connect(channel_addr)
            .await
            .expect("Error connecting to channel!");
        let mut channel_lines = Framed::new(stream, LinesCodec::new());
        if let Ok(encoded_msg) = serde_json::to_string(&UserMessage::Join { token : token.as_ref().unwrap().clone() }) {
            channel_lines.send(encoded_msg).await?;
        }
        

        let mut line = String::new();
        loop {
            tokio::select! {
                _ = ctrlc_channel.recv() => {
                    tracing::debug!("CTRL-C clicked, changing channel");
                    break;
                },
                _ = stdin.read_line(&mut line) => {
                    line.pop(); // remove end of line
                    if line.len() == 0 {
                        continue;
                    }
                    if let Ok(encoded_msg) = serde_json::to_string(&UserMessage::TextMessage{token : token.as_ref().unwrap().clone(), content : line.clone()}) {
                        channel_lines.send(encoded_msg).await?;
                        line.clear();
                    }
                }
                Some(Ok(ServerMessage::TextMessage { content })) = get_next_server_message(&mut channel_lines) => {
                    println!("{}", content);
                }
            }
        }
    }

    Ok(())
}

async fn connect_to_server() -> Framed<TcpStream, LinesCodec> {
    let server_address = SocketAddr::new(SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT);
    let socket = TcpSocket::new_v4().expect("Error opening socket for connection to server");
    let stream = socket
        .connect(server_address)
        .await
        .expect("Error connecting to server!");

    tracing::info!("Successfully connected to server {}", server_address);

    Framed::new(stream, LinesCodec::new())
}


fn ctrl_channel() -> Result<mpsc::UnboundedReceiver<()>, ctrlc::Error> {
    let (tx, rx) = mpsc::unbounded_channel();
    ctrlc::set_handler(move || {
        let _ = tx.send(());
    })?;

    Ok(rx)
}
