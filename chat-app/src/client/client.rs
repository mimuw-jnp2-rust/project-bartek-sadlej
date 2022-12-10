use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::database::AuthenticationToken;
use chat_app::messages::{ServerMessage, UserMessage};
use chat_app::utils::{get_next_server_message, send_to, ChatError};

use std::env;
use std::net::SocketAddr;
use std::process::exit;

use async_std::io::{self, WriteExt};

use futures::SinkExt;
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio_util::codec::{Framed, LinesCodec};

use anyhow::{bail, Context, Result};

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
            .context("Error in login")?
            .unwrap();

        handle_config(&mut server_lines, &token, &mut ctrlc_channel, &stdin).await?;

        let channel_addr = choose_channel(&mut server_lines, &stdin, &token).await?;

        let channel_lines = connect_to_channel(channel_addr, &token).await?;

        message_loop(channel_lines, &token, &mut ctrlc_channel, &stdin).await?;
    }
}

fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
}

async fn handle_config(
    server_lines: &mut Framed<TcpStream, LinesCodec>,
    token: &AuthenticationToken,
    ctrlc_channel: &mut UnboundedReceiver<()>,
    stdin: &io::Stdin,
) -> Result<()> {
    // Clear terminal
    clear_screen();

    loop {
        let mut line = String::new();
        print!(
            "
            =====================================\n
            = 0 - create new user               =\n
            = 1 - create new channel            =\n
            = 2 - choose channel                =\n
            = 3 - exit                          =\n
            =====================================\n
        "
        );
        stdin.read_line(&mut line).await?;
        tracing::debug!("inserted {}", line);
        match line.trim().parse::<i16>()? {
            0 => create_user(server_lines, token, ctrlc_channel, stdin).await?,
            1 => create_channel(server_lines, token, ctrlc_channel, stdin).await?,
            2 => return Ok(()),
            3 => exit(0),
            n => tracing::debug!("Invalid option {}", n),
        }
    }
}

async fn create_channel(
    server_lines: &mut Framed<TcpStream, LinesCodec>,
    token: &AuthenticationToken,
    ctrlc_channel: &mut UnboundedReceiver<()>,
    stdin: &io::Stdin,
) -> Result<()> {
    clear_screen();
    let mut name: Option<String> = None;
    let mut line = String::new();
    loop {
        clear_screen();
        if name.is_some() {
            println!(
                "Entered name: {}, write OK to continue or CTRL-C to change",
                name.as_ref().unwrap()
            );
        } else {
            println!("Enter channel name");
        }
        tokio::select! {
            _ = ctrlc_channel.recv() => {
                tracing::debug!("CTRL-C clicked in create channel");
                if name.is_none() {
                    return Ok(());
                } else {
                    name = None;
                }
            },
            _ = stdin.read_line(&mut line) => {
                line.pop(); // remove end of line
                if name.is_some() {
                    if line == "OK" {
                        break
                    } else {
                        name = None;
                    }
                } else {
                    name = Some(line.clone());
                }
            }
        }
        line.clear();
    }

    send_to(
        server_lines,
        &UserMessage::CreateChannel {
            token: token.clone(),
            name: name.unwrap(),
        },
    )
    .await?;

    if let Some(Ok(ServerMessage::TextMessage { content })) =
        get_next_server_message(server_lines).await
    {
        println!("{}", content);
    } else {
        tracing::debug!("Error creating channel");
    }

    Ok(())
}

async fn create_user(
    server_lines: &mut Framed<TcpStream, LinesCodec>,
    token: &AuthenticationToken,
    ctrlc_channel: &mut UnboundedReceiver<()>,
    stdin: &io::Stdin,
) -> Result<()> {
    let mut user_data: Option<(String, String)> = None;
    let mut line = String::new();
    loop {
        clear_screen();
        if user_data.is_some() {
            println!(
                "Entered name: {}, password: {}, write OK to continue or CTRL-C to change",
                user_data.as_ref().unwrap().0,
                user_data.as_ref().unwrap().1
            );
        } else {
            println!("Enter user name and password");
        }
        tokio::select! {
            _ = ctrlc_channel.recv() => {
                tracing::debug!("CTRL-C clicked in create channel");
                if user_data.is_none() {
                    return Ok(());
                } else {
                    user_data = None;
                }
            },
            _ = stdin.read_line(&mut line) => {
                line.pop(); // remove end of line
                if user_data.is_some() {
                    if line == "OK" {
                        break
                    } else {
                        user_data = None;
                    }
                } else {
                    let tmp : Vec<&str> = line.split(' ').collect();
                    if tmp.len() != 2 {
                        user_data = None;
                        continue;
                    };
                    user_data = Some((tmp[0].to_string(), tmp[1].to_string()));
                }
            }
        }
        line.clear();
    }

    let (name, password) = user_data.unwrap();
    send_to(
        server_lines,
        &UserMessage::CreateUser {
            token: token.clone(),
            name,
            password,
        },
    )
    .await?;

    if let Some(Ok(ServerMessage::TextMessage { content })) =
        get_next_server_message(server_lines).await
    {
        println!("{}", content);
    } else {
        tracing::debug!("Error creating new user");
    }

    Ok(())
}

fn setup_logging() -> Result<()> {
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
    server_lines: &mut Framed<TcpStream, LinesCodec>,
    user_name: &str,
    password: &str,
) -> Result<Option<AuthenticationToken>> {
    let connect_msg = serde_json::to_string(&UserMessage::Connect {
        name: user_name.to_string(),
        password: password.to_string(),
    })
    .context("error connecting to server!")?;

    server_lines.send(&connect_msg).await?;

    if let Some(Ok(ServerMessage::ConnectResponse {
        token: new_token,
        error: auth_error,
    })) = get_next_server_message(server_lines).await
    {
        if let Some(error_msg) = auth_error {
            println!("{}", error_msg);
            bail!(ChatError::InvalidPassword);
        }
        Ok(new_token)
    } else {
        tracing::info!("did not get channels list from server, try again...");
        Ok(None)
    }
}

async fn choose_channel(
    server_lines: &mut Framed<TcpStream, LinesCodec>,
    stdin: &io::Stdin,
    token: &AuthenticationToken,
) -> Result<SocketAddr> {
    send_to(
        server_lines,
        &UserMessage::GetChannels {
            token: token.clone(),
        },
    )
    .await
    .context("Error receiving channels list from server")?;
    if let Some(Ok(ServerMessage::ChannelsInfo {
        channels: channels_infos,
    })) = get_next_server_message(server_lines).await
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
                clear_screen();
                _ = io::stdout().flush().await;
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

    clear_screen();
    Ok(channel_lines)
}

async fn message_loop(
    mut channel_lines: Framed<TcpStream, LinesCodec>,
    token: &AuthenticationToken,
    ctrlc_channel: &mut UnboundedReceiver<()>,
    stdin: &io::Stdin,
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
