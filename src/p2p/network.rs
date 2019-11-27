use futures::future::{self, Either};
use futures::try_ready;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use crate::backend::operation::Operation;
use crate::p2p::{shared::Shared, node::Connection, node::Node};

pub type Tx = mpsc::Sender<Operation>;
pub type Rx = mpsc::Receiver<Operation>;

/// This is the gateway to the p2p network.
pub struct Network {
    state: Arc<Mutex<Shared>>,
}

impl Network {
    /// Create a new network object, and do setup
    /// @retval Rx The network-to-backend receive channel
    pub fn new() -> (Network, Rx) {
        let addr = "0.0.0.0:35010"
            .parse::<SocketAddr>()
            .expect("failed to parse nettwork address");
        let listener = TcpListener::bind(&addr).expect("failed to bind");

        let (tx, rx) = mpsc::channel();
        let shared = Shared::new(tx);

        let network = Network {
            state: Arc::new(Mutex::new(shared)),
        };

        let state = network.state.clone();
        thread::spawn(move || {
            Self::launch(state, listener);
        });

        (network, rx)
    }


    /// [ BLOCK AHEAD  |   BLOCK    ]
    ///    4 bytes          x bytes
    pub fn broadcast(&self, ) {

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
