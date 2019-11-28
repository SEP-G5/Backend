use crate::p2p::packet::{Packet, PacketErr};
use crate::p2p::{node::Node, shared::Shared};
use futures::executor::block_on;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

pub type Tx = mpsc::Sender<Packet>;
pub type Rx = mpsc::Receiver<Packet>;

/// This is the gateway to the p2p network.
pub struct Network {
    state: Arc<Mutex<Shared>>,
    n2b_rx: Rx,
}

impl Network {
    /// Create a new network object, and do setup
    /// @retval Rx The network-to-backend receive channel
    pub fn new() -> Network {
        let (tx, rx) = mpsc::channel(1337);
        let shared = Shared::new(tx);

        let network = Network {
            state: Arc::new(Mutex::new(shared)),
            n2b_rx: rx,
        };

        block_on(network.run());

        network
    }

    /// Try recv on the network-to-backend channel.
    pub async fn recv(&mut self) -> Option<Packet> {
        self.n2b_rx.recv().await
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
            match tx.send(packet.clone()).await {
                Ok(_) => {}
                Err(_) => println!("failed to send to node"),
            }
        }
    }

    async fn run(&self) {
        let addr = "0.0.0.0:35010"
            .parse::<SocketAddr>()
            .expect("failed to parse nettwork address");
        let mut listener = TcpListener::bind(&addr).await.expect("failed to bind");

        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = match listener.accept().await {
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
