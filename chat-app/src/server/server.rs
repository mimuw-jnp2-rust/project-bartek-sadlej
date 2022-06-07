use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use chat_app::channel::{Channel, ChannelInfo};
use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::database::{ChatDatabase, AuthenticationToken};
use chat_app::messages::{ServerMessage, UserMessage};
use chat_app::utils::{calculate_hash, get_next_user_message, send_to, ChatError};

use futures::SinkExt;
use tokio::net::{TcpListener, TcpStream};
use tokio_postgres::{tls::NoTlsStream, Client, Connection, NoTls, Socket};
use tokio_util::codec::{Framed, LinesCodec};

use futures::future;

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // // ssh -L 11212:lkdb:5432 bs429589@students.mimuw.edu.pl
    env::set_var("RUST_LOG", "debug");
    setup_logging()?;
    
    let chat_db = configure_database().await?;
    let channels_info_message = configure_channels(&chat_db).await?;
    let listener = configure_server().await?;

    accept_loop(listener, chat_db, channels_info_message).await?;
    Ok(())
}

async fn configure_database() -> Result<Arc<ChatDatabase>> {
    // "postgresql://bs429589:iks@localhost:11212/rust_db"
    let (client, connection) =
        tokio_postgres::connect("postgresql://bs429589:iks@localhost:11212/bd", NoTls)
            .await
            .context("Error connecting to database!")?;
    tracing::debug!("TU");

    tokio::spawn({
        async move {
            if let Err(e) = connection.await {
                tracing::error!("Error in database connection: {}", e)
            }
        }
    });

    // future::try_join(
    //     client.batch_execute("DROP TABLE IF EXISTS channels"),
    //     client.batch_execute("DROP TABLE IF EXISTS users")
    // ).await?;

    // future::try_join(
    //     client.batch_execute("CREATE TABLE channels (
    //         id          SERIAL PRIMARY KEY,
    //         name        VARCHAR NOT NULL)"),
    //     client.batch_execute("CREATE TABLE users (
    //         id          SERIAL PRIMARY KEY,
    //         name        VARCHAR NOT NULL,
    //         password    BIGINT)")
    //     ).await?;

    // client.execute("INSERT INTO channels (name) VALUES ($1)", &[&"RED"]).await?;
    // client.execute("INSERT INTO channels (name) VALUES ($1)", &[&"BLUE"]).await?;
    // client.execute("INSERT INTO channels (name) VALUES ($1)", &[&"BROWN"]).await?;

    // let name1= "Alice".to_string();
    // let name2= "Bob".to_string();

    // let password1: i64 = calculate_hash(&"123") as i64;
    // let password2 : i64= calculate_hash(&"456") as i64;

    // client.execute("INSERT INTO users (name, password) VALUES ($1, $2)", &[&name1, &password1]).await?;
    // client.execute("INSERT INTO users (name, password) VALUES ($1, $2)", &[&name2, &password2]).await?;

    Ok(Arc::new(ChatDatabase::new(client)))
}

fn setup_logging() -> Result<()> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("chat=info".parse()?))
        .with_span_events(FmtSpan::FULL)
        .init();
    Ok(())
}

async fn configure_channels(chat_db: &Arc<ChatDatabase>) -> Result<Arc<RwLock<ServerMessage>>> {
    let mut channels_infos: Vec<ChannelInfo> = Vec::new();
    for channel_name in chat_db.get_channels_names().await? {
        let chat_db = Arc::clone(chat_db);
        let new_channel = Arc::new(Channel::new(channel_name, chat_db).await);
        channels_infos.push(new_channel.get_channel_info());

        tokio::spawn(async move {
            if let Err(e) = new_channel.listen().await {
                tracing::info!("an error occurred; error = {:?}", e);
            }
        });
    }
    tracing::info!("Created channels: {:?}", channels_infos);
    let channels_info_message = Arc::new(RwLock::new(ServerMessage::ChannelsInfo {
        channels: channels_infos,
    }));
    Ok(channels_info_message)
}

async fn configure_server() -> Result<TcpListener> {
    let server_address = SocketAddr::new(SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT);
    let listener = TcpListener::bind(server_address)
        .await
        .expect("[MAIN_SERVER] Error starting server!");
    tracing::info!("[MAIN_SERVER] Server running on {}", server_address);
    Ok(listener)
}

async fn accept_loop(
    listener: TcpListener,
    chat_db: Arc<ChatDatabase>,
    channels_info_message: Arc<RwLock<ServerMessage>>,
) -> Result<()> {
    loop {
        let (stream, addr) = listener.accept().await.context("Error in accept loop!")?;

        let chat_db = Arc::clone(&chat_db);
        let channels_info_message = Arc::clone(&channels_info_message);

        tokio::spawn(async move {
            tracing::info!("[MAIN_SERVER] accepted connection {}", addr);
            if let Err(e) = handle_new_user(stream, addr, chat_db, channels_info_message).await {
                tracing::info!("[MAIN_SERVER] an error occurred; error = {:?}", e);
            }
        });
    }
}

async fn authorize_connection(
    chat_db: &Arc<ChatDatabase>,
    token: &AuthenticationToken,
    lines: &mut Framed<TcpStream, LinesCodec>
) -> Result<()> {
    if chat_db.authorize_connection(token, &lines.get_ref().peer_addr().unwrap()) {
        Ok(())
    } else {
        let msg_text = "Unauthirized connection! Disconnecting".to_string();
        tracing::debug!("{}", msg_text);
        send_to(lines, &ServerMessage::TextMessage { content:msg_text }).await?;
        anyhow::private::Err(anyhow::Error::new(ChatError::UnauthenticatedConnection))
    }
}

// Authenticates user and send them channels info
async fn handle_new_user(
    stream: TcpStream,
    addr: SocketAddr,
    chat_db: Arc<ChatDatabase>,
    channels_info_message: Arc<RwLock<ServerMessage>>,
) -> Result<()> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    match get_next_user_message(&mut lines).await {
        Some(Ok(UserMessage::Connect { name, password })) => {
            if let Ok(token) = chat_db.authenticate_user(name, password, addr) {
                send_to(
                    &mut lines,
                    &ServerMessage::ConnectResponse {
                        token: Some(token),
                        error: None,
                    }
                ).await?;
            }
        }
        _ => {
            tracing::error!(
                "[MAIN_SERVER] Failed to get connect message. Client {} disconnected.",
                addr
            );
            return Ok(())
        }
    };

    loop {
        match get_next_user_message(&mut lines).await {
            Some(Ok(UserMessage::CreateChannel {
                token,
                name
            })) =>  {
                authorize_connection(&chat_db, &token, &mut lines).await?;
                let content : String;
                match chat_db.create_channel(&name).await {
                    Ok(()) => content = format!("Successfully created channel {}", name),
                    Err(e) => content = format!("{:?}", e),
                }
                send_to(&mut lines, &ServerMessage::TextMessage { content }).await?;
            }
            Some(Ok(UserMessage::GetChannels {
                token: AuthenticationToken,
            })) => {
                send_to(&mut lines, channels_info_message.as_ref()).await?;
                break;
            }
            _ => tracing::debug!("Unimpleneted"),
        }
    } 

    Ok(())
}

async fn create_channel(
    stream: TcpStream,
    addr: SocketAddr,
    chat_db: Arc<ChatDatabase>,
    channels_info_message: Arc<RwLock<ServerMessage>>,   
) -> Result<()> {
    todo!();
}
