use crate::p2p::packet::Packet;
use crate::p2p::{node::Node, shared::Shared};
use futures::executor::block_on;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};

pub type Tx = mpsc::Sender<(Packet, Option<SocketAddr>)>;
pub type Rx = mpsc::Receiver<(Packet, Option<SocketAddr>)>;

type StdRx = std::sync::mpsc::Receiver<(Packet, Option<SocketAddr>)>;
type StdTx = std::sync::mpsc::Sender<(Packet, Option<SocketAddr>)>;

/// This is the gateway to the p2p network.
pub struct Network {
    state: Arc<Mutex<Shared>>,
    //n2b_rx: Rx,
    n2b_rx: StdRx,
}

impl Network {
    /// Create a new network object, and do setup
    /// @retval Rx The network-to-backend receive channel
    pub fn new() -> Network {
        let (tx, rx) = mpsc::channel(1337);
        let (stdtx, stdrx) = std::sync::mpsc::channel();
        let shared = Shared::new(tx);
        let network = Network {
            state: Arc::new(Mutex::new(shared)),
            n2b_rx: stdrx,
        };

        block_on(network.run(rx, stdtx));
        network
    }

    /// Try recv on the network-to-backend channel.
    pub fn try_recv(&mut self) -> Option<(Packet, SocketAddr)> {
        match self.n2b_rx.try_recv() {
            Ok((p, a)) => Some((p, a.expect("SocketAddr must always be 'Some' here"))),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(_) => panic!("n2b_rx channel broken"),
        }
    }

    pub fn broadcast(&self, packet: Packet) {
        block_on(self.broadcast_internal(packet));
    }

    /// Attempt to broadcast the packet to all connected nodes.
    pub async fn broadcast_internal(&self, packet: Packet) {
        println!("broadcasting packet");
        let nodes = &mut self.state.lock().await.b2n_tx;
        for (addr, tx) in nodes.iter_mut() {
            println!("sending to {:?}", addr);
            match tx.send((packet.clone(), None)).await {
                Ok(_) => {}
                Err(_) => println!("failed to send to node"),
            }
        }
    }

    pub fn unicast(&self, packet: Packet, addr: SocketAddr) {
        block_on(self.unicast_internal(packet, addr));
    }

    pub async fn unicast_internal(&self, packet: Packet, addr: SocketAddr) {
        println!("unicasting packet");
        let nodes = &mut self.state.lock().await.b2n_tx;
        if let Some(tx) = nodes.get_mut(&addr) {
            match tx.send((packet.clone(), None)).await {
                Ok(_) => {}
                Err(_) => println!("failed to send to node"),
            }
        }
    }

    async fn run(&self, mut rx: Rx, stdtx: StdTx) {
        let addr = "0.0.0.0:35010"
            .parse::<SocketAddr>()
            .expect("failed to parse nettwork address");
        let mut listener = TcpListener::bind(&addr).await.expect("failed to bind");

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(packet) => stdtx.send(packet).expect("n2b_rx channel broken"),
                    None => (),
                }
            }
        });

        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(b) => b,
                    Err(e) => panic!("listener socket error {:?}", e),
                };

                let state = state.clone();
                tokio::spawn(async move {
                    let mut node = Node::new(stream, state).await;
                    node.run().await;
                });
            }
        });
    }
}
