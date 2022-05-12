use std::{net::{SocketAddr}, sync::Arc, io, collections::HashSet};

use tokio::net::TcpListener;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, net::TcpStream};
use tokio_util::codec::{Framed, LinesCodec};
use std::error::Error;

use crate::{config::SERVER_DEFAULT_IP_ADDRESS, database::ChatDatabase};


type Tx = mpsc::UnboundedSender<String>;
type Rx = mpsc::UnboundedReceiver<String>;

pub struct Channel {
    name : String,
    listener : TcpListener,
    shared : Arc<Shared>,
}

impl Channel {
    pub async fn new(name : String, chat_db : Arc<ChatDatabase>) -> Channel {
        let free_port = portpicker::pick_unused_port().expect("No ports free");
        let listener = TcpListener::bind((SERVER_DEFAULT_IP_ADDRESS, free_port)).await.expect(&format!("Error starting channel {}!", name));
        Channel {
            name,
            listener,
            shared : Arc::new(Shared::new(chat_db)),
        }
    }

    pub fn get_channel_info(&self) -> ChannelInfo {
        ChannelInfo {
            name : self.name.clone(),
            address : self.listener.local_addr().expect("Error converting channel address to std::net::SocketAddr"),
        }
    }

    pub async fn listen(self: Arc<Self>) -> Result<(), Box<dyn Error>> {

        loop {
            let me = Arc::clone(&self);
            let (stream, addr) = me.to_owned().listener.accept().await.unwrap();
            
            let state = Arc::clone(&me.shared);
    
            tokio::spawn(async move {
                tracing::debug!("[{}] accepted connection {:?}", me.name, addr);
                if let Err(e) = Channel::handle_connection(me.name.as_str(), state, stream, addr).await {
                    tracing::info!("[{}] an error occurred; error = {:?}", me.name, e);
                }
            });
        }
    }

    async fn handle_connection(
        name : &str,
        state: Arc<Shared>,
        stream: TcpStream,
        addr: SocketAddr) -> Result<(), Box<dyn Error>> {
            Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChannelInfo {
    name : String,
    address : std::net::SocketAddr,
}



#[derive(Debug)]
struct Shared {
    peers: DashMap<SocketAddr, Tx>,
    chat_db : Arc<ChatDatabase>
}

impl Shared {
    fn new(chat_db : Arc<ChatDatabase>) -> Self {
        Shared {
            peers: DashMap::new(),
            chat_db : chat_db,
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
    rx: Rx,
}

impl Peer {
    async fn new(state: Arc<Shared>, lines: Framed<TcpStream, LinesCodec>) -> io::Result<Peer> {
        let addr = lines.get_ref().peer_addr()?;

        let (tx, rx) = mpsc::unbounded_channel();

        state.peers.insert(addr, tx);

        Ok(Peer { lines, rx })
    }
}