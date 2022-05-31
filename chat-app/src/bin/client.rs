use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::database::AuthenticationToken;
use chat_app::messages::{ServerMessage, UserMessage};
use chat_app::utils::{get_next_server_message, ChatError};

use std::env;
use std::net::SocketAddr;

use async_std::io;

use futures::SinkExt;
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio_util::codec::{Framed, LinesCodec};

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {

    env::set_var("RUST_LOG", "debug");
    setup_logging()?;
    let (user_name, password) = parse_args().context("usage: [name] [password]")?;

    let mut token: AuthenticationToken;
    let mut ctrlc_channel = ctrl_channel().context("error seting ctrl-c actions")?;
    let stdin = io::stdin();

    loop {
        let mut server_lines = connect_to_server().await;
        token = login(&mut server_lines, &user_name, &password)
            .await
            .context("Error in login")?.unwrap();

        let channel_addr = choose_channel(&mut server_lines, &stdin).await?;

        let channel_lines = connect_to_channel(channel_addr, &token).await?;

        message_loop(channel_lines, &token, &mut ctrlc_channel, &stdin).await?;
    }
}

fn setup_logging() -> Result<()>{
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("chat=info".parse()?))
        .with_span_events(FmtSpan::FULL)
        .init();
    Ok(())
}

fn parse_args() -> Result<(String, String)> {
    let user_name = env::args()
        .nth(1)
        .context("provide name as first argument")?;
    let password = env::args()
        .nth(2)
        .context("provide password as second argument")?;

    Ok((user_name, password))
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

async fn login(
    mut server_lines: &mut Framed<TcpStream, LinesCodec>,
    user_name: &String,
    password: &String,
) -> Result<Option<AuthenticationToken>> {
    let connect_msg = serde_json::to_string(&UserMessage::Connect {
        name: user_name.clone(),
        password: password.clone(),
    })
    .context("error connecting to server!")?;

    server_lines.send(&connect_msg).await?;

    if let Some(Ok(ServerMessage::ConnectResponse {
        token: new_token,
        error: auth_error,
    })) = get_next_server_message(&mut server_lines).await
    {
        if let Some(error_msg) = auth_error {
            println!("{}", error_msg);
        }
        Ok(new_token)
    } else {
        tracing::info!("did not get channels list from server, try again...");
        Ok(None)
    }
}

async fn choose_channel(
    mut server_lines: &mut Framed<TcpStream, LinesCodec>,
    stdin : &io::Stdin,
) -> Result<SocketAddr> {
    if let Some(Ok(ServerMessage::ChannelsInfo {
        channels: channels_infos,
    })) = get_next_server_message(&mut server_lines).await
    {
        println!("Choose channel");
        for (idx, channel_info) in channels_infos.iter().enumerate() {
            println!("[{}] {}", idx, channel_info.name);
        }

        
        loop {
            let mut line = String::new();
            println!("enter number in [0 ... {}]", channels_infos.len() - 1);
            stdin.read_line(&mut line).await?;
            let channel_nr = line.trim().parse::<usize>()?;
            if channel_nr < channels_infos.len() {
                return Ok(channels_infos[channel_nr].address);
            }
        }
    } else {
        tracing::info!("Error receiving channels info from server");
        Err(anyhow::Error::from(ChatError::InvalidMessage))
    }
}

async fn connect_to_channel(
    channel_addr: SocketAddr,
    token: &AuthenticationToken,
) -> Result<Framed<TcpStream, LinesCodec>> {
    let channel_socket =
        TcpSocket::new_v4().context("Error opening socket for connection to channel")?;
    let stream = channel_socket
        .connect(channel_addr)
        .await
        .context("Error connecting to channel!")?;
    let mut channel_lines = Framed::new(stream, LinesCodec::new());
    if let Ok(encoded_msg) = serde_json::to_string(&UserMessage::Join {
        token: token.clone(),
    }) {
        channel_lines.send(encoded_msg).await?;
    }

    Ok(channel_lines)
}

async fn message_loop(mut channel_lines: Framed<TcpStream, LinesCodec>, token: &AuthenticationToken, ctrlc_channel : &mut UnboundedReceiver<()>,     stdin : &io::Stdin,
) -> Result<()> {
    let mut line = String::new();
    loop {
        tokio::select! {
            _ = ctrlc_channel.recv() => {
                tracing::debug!("CTRL-C clicked, changing channel");
                break;
            },
            _ = stdin.read_line(&mut line) => {
                line.pop(); // remove end of line
                if line.is_empty() {
                    continue;
                }
                if let Ok(encoded_msg) = serde_json::to_string(&UserMessage::TextMessage{token : token.clone(), content : line.clone()}) {
                    channel_lines.send(encoded_msg).await?;
                    line.clear();
                }
            }
            Some(Ok(ServerMessage::TextMessage { content })) = get_next_server_message(&mut channel_lines) => {
                println!("{}", content);
            }
        }
    }
    Ok(())
}
