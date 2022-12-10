use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use chat_app::channel::{Channel, ChannelInfo};
use chat_app::config::{SERVER_DEFAULT_IP_ADDRESS, SERVER_DEFAULT_PORT};
use chat_app::database::{AuthenticationToken, ChatDatabase};
use chat_app::messages::{ServerMessage, UserMessage};
use chat_app::utils::{calculate_hash, get_next_user_message, send_to, ChatError};

use tokio::net::{TcpListener, TcpStream};
use tokio_postgres::NoTls;
use tokio_util::codec::{Framed, LinesCodec};

use futures::future;

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // // ssh -L 11212:lkdb:5432 bs429589@students.mimuw.edu.pl
    env::set_var("RUST_LOG", "debug");
    setup_logging()?;

    let chat_db = configure_database().await?;
    let channels_infos: Arc<RwLock<Vec<ChannelInfo>>> = Arc::new(RwLock::new(Vec::new()));
    configure_channels(&chat_db, &channels_infos, None).await?;
    let listener = configure_server().await?;

    accept_loop(listener, chat_db, channels_infos).await?;
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

    client
        .batch_execute("DROP TABLE IF EXISTS channels CASCADE")
        .await?;
    client
        .batch_execute("DROP TABLE IF EXISTS users CASCADE")
        .await?;
    client
        .batch_execute("DROP TABLE IF EXISTS messages CASCADE")
        .await?;
    client
        .batch_execute("DROP TABLE IF EXISTS history CASCADE")
        .await?;

    future::try_join(
        client.batch_execute(
            "CREATE TABLE channels (
            name        TEXT NOT NULL PRIMARY KEY)",
        ),
        client.batch_execute(
            "CREATE TABLE users (
            name        TEXT NOT NULL PRIMARY KEY,
            password    BIGINT)",
        ),
    )
    .await?;

    client
        .batch_execute(
            "CREATE TABLE messages (
        id              SERIAL PRIMARY KEY,
        channel_name    TEXT NOT NULL,
        user_name       TEXT NOT NULL,
        content         TEXT,
        CONSTRAINT      fk_channel FOREIGN KEY(channel_name) REFERENCES channels(name),
        CONSTRAINT      fk_user FOREIGN KEY(user_name) REFERENCES users(name)
    )",
        )
        .await?;

    client
        .batch_execute(
            "CREATE TABLE history (
        user_name       TEXT NOT NULL,
        channel_name    TEXT NOT NULL,
        message_id      INT NOT NULL,
        CONSTRAINT      pk PRIMARY KEY(user_name, channel_name),
        CONSTRAINT      fk_message FOREIGN KEY(message_id) REFERENCES messages(id)
    )",
        )
        .await?;

    client
        .execute("INSERT INTO channels (name) VALUES ($1)", &[&"RED"])
        .await?;
    client
        .execute("INSERT INTO channels (name) VALUES ($1)", &[&"BLUE"])
        .await?;
    client
        .execute("INSERT INTO channels (name) VALUES ($1)", &[&"BROWN"])
        .await?;

    let name1 = "ADMIN".to_string();

    let password1: i64 = calculate_hash(&"ADMIN") as i64;

    client
        .execute(
            "INSERT INTO users (name, password) VALUES ($1, $2)",
            &[&name1, &password1],
        )
        .await?;

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

async fn configure_channels(
    chat_db: &Arc<ChatDatabase>,
    channels_infos: &Arc<RwLock<Vec<ChannelInfo>>>,
    mut new_channels_names: Option<Vec<String>>,
) -> Result<()> {
    if new_channels_names.is_none() {
        new_channels_names = Some(
            chat_db
                .get_channels_names()
                .await
                .context("Error getting channels names from db")?,
        );
    } else {
        for name in new_channels_names.as_ref().unwrap() {
            chat_db.create_channel(name).await?;
        }
    }

    for channel_name in new_channels_names.unwrap() {
        let chat_db = Arc::clone(chat_db);
        let new_channel = Arc::new(Channel::new(channel_name, chat_db).await);
        let new_channel_info = new_channel.get_channel_info();
        tracing::info!("Created channel: {:?}", new_channel_info);
        channels_infos.write().unwrap().push(new_channel_info);

        tokio::spawn(async move {
            if let Err(e) = new_channel.listen().await {
                tracing::info!("an error occurred; error = {:?}", e);
            }
        });
    }

    Ok(())
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
    channels_infos: Arc<RwLock<Vec<ChannelInfo>>>,
) -> Result<()> {
    loop {
        let (stream, addr) = listener.accept().await.context("Error in accept loop!")?;

        let chat_db = Arc::clone(&chat_db);
        let channels_infos = Arc::clone(&channels_infos);

        tokio::spawn(async move {
            tracing::info!("[MAIN_SERVER] accepted connection {}", addr);
            if let Err(e) = handle_new_user(stream, addr, chat_db, channels_infos).await {
                tracing::info!("[MAIN_SERVER] an error occurred; error = {:?}", e);
            }
        });
    }
}

async fn authorize_connection(
    chat_db: &Arc<ChatDatabase>,
    token: &AuthenticationToken,
    lines: &mut Framed<TcpStream, LinesCodec>,
) -> Result<()> {
    if chat_db.authorize_connection(token) {
        Ok(())
    } else {
        let msg_text = "Unauthirized connection! Disconnecting".to_string();
        tracing::debug!("{}", msg_text);
        send_to(lines, &ServerMessage::TextMessage { content: msg_text }).await?;
        anyhow::private::Err(anyhow::Error::new(ChatError::UnauthenticatedConnection))
    }
}

// Authenticates user and send them channels info
async fn handle_new_user(
    stream: TcpStream,
    addr: SocketAddr,
    chat_db: Arc<ChatDatabase>,
    channels_infos: Arc<RwLock<Vec<ChannelInfo>>>,
) -> Result<()> {
    let mut lines = Framed::new(stream, LinesCodec::new());

    match get_next_user_message(&mut lines).await {
        Some(Ok(UserMessage::Connect { name, password })) => {
            match chat_db.authenticate_user(&name, &password).await {
                Ok(token) => {
                    send_to(
                        &mut lines,
                        &ServerMessage::ConnectResponse {
                            token: Some(token),
                            error: None,
                        },
                    )
                    .await?;
                }
                Err(err) => {
                    send_to(
                        &mut lines,
                        &ServerMessage::ConnectResponse {
                            token: None,
                            error: Some(err.to_string()),
                        },
                    )
                    .await?;
                }
            }
        }
        _ => {
            tracing::error!(
                "[MAIN_SERVER] Failed to get connect message. Client {} disconnected.",
                addr
            );
            return Ok(());
        }
    }

    loop {
        match get_next_user_message(&mut lines).await {
            Some(Ok(UserMessage::CreateChannel { token, name })) => {
                authorize_connection(&chat_db, &token, &mut lines).await?;
                let content =
                    match configure_channels(&chat_db, &channels_infos, Some(vec![name.clone()]))
                        .await
                    {
                        Ok(()) => format!("Successfully created channel {}", name),
                        Err(e) => format!("{:?}", e),
                    };
                send_to(&mut lines, &ServerMessage::TextMessage { content }).await?;
            }
            Some(Ok(UserMessage::CreateUser {
                token,
                name,
                password,
            })) => {
                authorize_connection(&chat_db, &token, &mut lines).await?;
                let content = match create_user(&chat_db, &name, &password).await {
                    Ok(()) => {
                        format!(
                            "Successfully created new user {} with password {}",
                            name, password
                        )
                    }
                    Err(e) => format!("{:?}", e),
                };
                send_to(&mut lines, &ServerMessage::TextMessage { content }).await?;
            }
            Some(Ok(UserMessage::GetChannels { token })) => {
                authorize_connection(&chat_db, &token, &mut lines).await?;
                let channels_info_message = ServerMessage::ChannelsInfo {
                    channels: channels_infos.read().unwrap().to_vec(),
                };
                send_to(&mut lines, channels_info_message).await?;
                break;
            }
            None => {
                tracing::info!("Client {:?} disconnected", addr);
                break;
            }
            _ => tracing::debug!("Unimpleneted"),
        }
    }

    Ok(())
}

async fn create_user(chat_db: &Arc<ChatDatabase>, name: &str, password: &str) -> Result<()> {
    chat_db.create_user(name, password).await?;
    Ok(())
}
