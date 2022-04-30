use std::net::IpAddr;

use async_std::net::TcpListener;

use crate::config::SERVER_DEFAULT_IP_ADDRESS;


type Tx = mpsc::UnboundedSender<String>;
type Rx = mpsc::UnboundedReceiver<String>;

pub struct Channel {
    name : String,
    listener : TcpListener,
    shared : Arc<Shared>,
    userNames : Arc<Set<String>>, // on purpose not in shared
}

impl Channel {
    pub fn new(name : String) {
        let free_port = portpicker::pick_unused_port().expect("No ports free");
        let listener = TcpListener::bind((SERVER_DEFAULT_IP_ADDRESS, free_port)).await.expect(format!("Error starting channel {}!", name).into());
        Channel {
            name,
            listener,
            shared : Arc::new(Shared::new()),
            userNames : Arc::new(Set::new()),
        };
    }

    pub fn get_channel_info() {
        ChannelInfo {
            name,
            address : listener.into()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChannelInfo {
    name : String,
    address : (IpAddr, u16),
}

#[derive(Debug)]
struct Shared {
    peers: DashMap<SocketAddr, Tx>,
}

impl Shared {
    fn new() -> Self {
        Shared {
            peers: DashMap::new(),
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