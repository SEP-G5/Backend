use crate::p2p::{node::Connection, node::Node, shared::Shared};
use futures::future::{self, Either};
use futures::try_ready;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use crate::p2p::packet::Packet;

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
        let addr = "0.0.0.0:35010"
            .parse::<SocketAddr>()
            .expect("failed to parse nettwork address");
        let listener = TcpListener::bind(&addr).expect("failed to bind");

        let (tx, rx) = mpsc::channel();
        let shared = Shared::new(tx);

        let network = Network {
            state: Arc::new(Mutex::new(shared)),
            n2b_rx: rx,
        };

        let state = network.state.clone();
        thread::spawn(move || {
            Self::launch(state, listener);
        });

        network
    }

    /// Try recv on the network-to-backend channel.
    pub fn try_recv(&self) -> Result<Packet, std::sync::mpsc::TryRecvError> {
        self.n2b_rx.try_recv()
    }

    /// Attempt to broadcast the packet to all connected nodes.
    pub fn broadcast(&self, packet: &Packet) {
        println!("broadcasting packet");
        let l = self.state.lock().expect("failed to lock shared state");
        println!("len {}", l.b2n_tx.len());
        for (addr, tx) in l.b2n_tx.iter() {
            println!("sending to {:?}", addr);
            match tx.send(packet.clone()) {
                Ok(_) => {},
                Err(_) => println!("failed to send to node"),
            }
        }
    }

    fn launch(state: Arc<Mutex<Shared>>, listener: TcpListener) {
        let server = listener
            .incoming()
            .map_err(|e| println!("error {:?}", e))
            .for_each(move |socket| {
                on_connection(socket, state.clone());
                Ok(())
            });
        tokio::run(server);
    }
}

fn on_connection(stream: TcpStream, state: Arc<Mutex<Shared>>) {
    let connection = Connection::new(stream);

    let fut = connection
        .into_future()
        .map_err(|(e, _)| e)
        .and_then(|(a, connection)| {
            let node = Node::new(connection, state);
            println!("new connection: {:?}", node.get_addr());
            node
        })
        .map_err(|e| {
            println!("connection error [{:?}]", e);
        });

    tokio::spawn(fut);
}
