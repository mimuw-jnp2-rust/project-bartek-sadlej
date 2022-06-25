use std::{io, net::SocketAddr, sync::Arc};

use dashmap::DashMap;
use futures::SinkExt;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_util::codec::{Framed, LinesCodec};

use anyhow::{Context, Result};

use crate::utils::send_to;
use crate::{
    config::SERVER_DEFAULT_IP_ADDRESS,
    database::ChatDatabase,
    messages::{ServerMessage, UserMessage},
    utils::{get_next_user_message, ChatError},
};

type Tx = mpsc::UnboundedSender<String>;
type Rx = mpsc::UnboundedReceiver<String>;

#[derive(Debug)]
pub struct Channel {
    name: String,
    listener: TcpListener,
    shared: Arc<Shared>,
}

impl Channel {
    pub async fn new(name: String, chat_db: Arc<ChatDatabase>) -> Channel {
        let free_port = portpicker::pick_unused_port().expect("No ports free");
        let listener = TcpListener::bind((SERVER_DEFAULT_IP_ADDRESS, free_port))
            .await
            .unwrap_or_else(|_| panic!("Error starting channel {}!", name));
        Channel {
            name,
            listener,
            shared: Arc::new(Shared::new(chat_db)),
        }
    }

    pub fn get_channel_info(&self) -> ChannelInfo {
        ChannelInfo {
            name: self.name.clone(),
            address: self
                .listener
                .local_addr()
                .expect("Error converting channel address to std::net::SocketAddr"),
        }
    }

    pub async fn listen(self: Arc<Self>) -> Result<()> {
        loop {
            let me = Arc::clone(&self);
            let (stream, addr) = me
                .to_owned()
                .listener
                .accept()
                .await
                .context(format!("[channel {}] Error in accept loop", me.name))?;

            let state = Arc::clone(&me.shared);

            tokio::spawn(async move {
                tracing::debug!("[{}] accepted connection {:?}", me.name, addr);
                if let Err(e) = Channel::handle_connection(&me.name, state, stream, addr).await {
                    tracing::info!("[{}] an error occurred; error = {:?}", me.name, e);
                }
            });
        }
    }

    async fn handle_connection(
        name: &str,
        state: Arc<Shared>,
        stream: TcpStream,
        addr: SocketAddr,
    ) -> Result<(), ChatError> {
        let mut lines = Framed::new(stream, LinesCodec::new());

        let user_name = match get_next_user_message(&mut lines).await {
            Some(Ok(UserMessage::Join { token })) => {
                if state.chat_db.authorize_connection(&token) {
                    tracing::info!("[{}] new authenticated connection from {}", name, addr);
                    token.user_name.clone()
                } else {
                    tracing::info!("[{}] unauthenticated connection from {}", name, addr);
                    return Err(ChatError::UnauthenticatedConnection);
                }
            }
            _ => return Err(ChatError::InvalidMessage),
        };

        Channel::send_unseen_messages(&mut lines, &state, name, &user_name).await?;

        let mut peer = Peer::new(state.clone(), lines)
            .await
            .map_err(|_| ChatError::RuntimeError)?;
        match serde_json::to_string(&ServerMessage::TextMessage {
            content: format!("{} has joined!", user_name),
        }) {
            Ok(msg) => state.broadcast(addr, &msg).await,
            _ => return Err(ChatError::RuntimeError),
        };

        loop {
            tokio::select! {
                Some(channel_member_message) = peer.rx.recv() => {
                    peer.lines.send(&channel_member_message).await.map_err(|_| ChatError::RuntimeError)?;
                }
                user_message = get_next_user_message(&mut peer.lines) => match user_message {
                    Some(Ok(UserMessage::TextMessage { token , content  })) => {
                        if state.chat_db.authorize_connection(&token) {
                            Channel::save_message(&state, name, &user_name, &content).await?;
                            let formetted_message = format!("[{}] {}", token.user_name, content);
                            if let Ok(encoded_message) = serde_json::to_string(&ServerMessage::TextMessage{content : formetted_message}) {
                                state.broadcast(addr, &encoded_message).await;
                            }
                        }
                        else {
                            tracing::info!("[{}] unauthenticated connection from {}",name, addr);
                            return Err(ChatError::UnauthenticatedConnection);
                        }
                    },
                    _ => {
                        Channel::save_history(&state, name, &user_name).await?;
                        tracing::info!("[{}] invalid message from {}, disconnecting",name, addr);
                        return Err(ChatError::InvalidMessage);
                    },
                },
            }
        }
    }

    async fn save_message(
        state: &Arc<Shared>,
        channel_name: &str,
        user_name: &str,
        message: &str,
    ) -> Result<(), ChatError> {
        state
            .chat_db
            .save_message(channel_name, user_name, message)
            .await
    }

    async fn save_history(
        state: &Arc<Shared>,
        channel_name: &str,
        user_name: &str,
    ) -> Result<(), ChatError> {
        state.chat_db.save_history(channel_name, user_name).await
    }

    async fn send_unseen_messages(
        lines: &mut Framed<TcpStream, LinesCodec>,
        state: &Arc<Shared>,
        channel_name: &str,
        user_name: &str,
    ) -> Result<()> {
        let unseen_messages = state
            .chat_db
            .get_unseed_messages(channel_name, user_name)
            .await?;
        for (user, content) in unseen_messages.into_iter() {
            let message = format!("[{}] {}", user, content);
            send_to(lines, &ServerMessage::TextMessage { content: message }).await?
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChannelInfo {
    pub name: String,
    pub address: std::net::SocketAddr,
}

#[derive(Debug)]
struct Shared {
    peers: DashMap<SocketAddr, Tx>,
    chat_db: Arc<ChatDatabase>,
}

impl Shared {
    fn new(chat_db: Arc<ChatDatabase>) -> Self {
        Shared {
            peers: DashMap::new(),
            chat_db,
        }
    }

    async fn broadcast(&self, sender: SocketAddr, message: &str) {
        for peer in self.peers.iter_mut() {
            if *peer.key() != sender {
                let _ = peer.value().send(message.into());
            }
        }
    }
}

struct Peer {
    lines: Framed<TcpStream, LinesCodec>,
    state: Arc<Shared>,
    rx: Rx,
}

impl Peer {
    async fn new(state: Arc<Shared>, lines: Framed<TcpStream, LinesCodec>) -> io::Result<Peer> {
        let addr = lines.get_ref().peer_addr()?;

        let (tx, rx) = mpsc::unbounded_channel();

        state.peers.insert(addr, tx);

        Ok(Peer { lines, state, rx })
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.state
            .peers
            .remove(&self.lines.get_ref().peer_addr().unwrap());
    }
}
