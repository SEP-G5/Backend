use bytes::{BufMut, Bytes, BytesMut};
use futures::future::{self, Either};
use futures::try_ready;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use crate::backend::operation::Operation;

type Tx = mpsc::Sender<Operation>;
type Rx = mpsc::Receiver<Operation>;

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

struct Shared {
    /// Maps nodes to the send half of the backend-to-network channel
    pub b2n_tx: HashMap<SocketAddr, Tx>,

    /// network-to-backend channel, for use by the nodes when sending to backend.
    pub n2b_tx: Tx,
}

impl Shared {
    fn new(tx: Tx) -> Shared {
        Shared {
            b2n_tx: HashMap::new(),
            n2b_tx: tx,
        }
    }
}

struct Node {
    addr: SocketAddr,
    con: Connection,
    state: Arc<Mutex<Shared>>,

    /// Receive half of the backend-to-network channel
    b2n_rx: Rx,
}

impl Node {
    fn new(con: Connection, state: Arc<Mutex<Shared>>, b2n_rx: Rx) -> Node {
        Node {
            addr: con
                .stream
                .peer_addr()
                .expect("failed to get peer address"),
            con,
            state,
            b2n_rx,
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.state.lock().unwrap().b2n_tx.remove(&self.addr);
    }
}

impl Future for Node {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        // Read messages from backend
        const MAX_POLLS: usize = 10;
        for i in 0..MAX_POLLS {}

        // Read messages from other nodes
        while let Async::Ready(can_read) = self.con.poll()? {

            match can_read {
                Some(true) => {
                    println!("got packet {:?} from {:?}", self.con.get_rd(), self.addr);
                    // TODO do stuff with self.con.get_rd() buffer
                    // ...

                    let msg = format!("hey there from node [{:?}]", self.addr);
                    self.con.stream.write(msg.as_bytes()).expect("could not write to connection");
                },
                Some(false) => {
                    return Ok(Async::NotReady);
                },
                None => {
                    return Ok(Async::Ready(()));
                }
            }
        }

        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
struct Connection {
    pub stream: TcpStream,
    pub rd: BytesMut,
    pub wr: BytesMut,
}

impl Connection {
    fn new(stream: TcpStream) -> Connection {
        let mut con = Connection { stream, rd: BytesMut::new(), wr: BytesMut::new() };
        con.rd.reserve(1024*1024);
        con.wr.reserve(1024*1024);
        con
    }

    fn get_rd(&self) -> &BytesMut {
        &self.rd
    }

    fn get_wr(&self) -> &BytesMut {
        &self.wr
    }

    fn fill_read_buf(&mut self) -> Poll<(), io::Error> {

        self.rd.clear();
        loop {
            let n = try_ready!(self.stream.read_buf(&mut self.rd));
            if n == 0 {
                return Ok(Async::Ready(()));
            }
        }
    }
}

impl Stream for Connection {
    type Item = bool;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let stream_closed = self.fill_read_buf()?.is_ready();

        if stream_closed {
            return Ok(Async::Ready(None));
        } else {
            if self.rd.len() > 0 {
                return Ok(Async::Ready(Some(true)));
            } else {
                return Ok(Async::Ready(Some(false)));
            }
        }
    }
}

fn on_connection(stream: TcpStream, state: Arc<Mutex<Shared>>) {
    let connection = Connection::new(stream);

    let fut = connection
        .into_future()
        .map_err(|(e, _)| e)
        .and_then(|(a, connection)| {
            let (b2n_tx, b2n_rx) = mpsc::channel();
            let node = Node::new(connection, state, b2n_rx);
            {
                let mut l = node.state.lock().expect("failed to aquire lock");
                l.b2n_tx.insert(node.addr.clone(), b2n_tx);
            }
            println!("new connection: {:?}", node.addr);
            node
        })
        .map_err(|e| {
            println!("connection error [{:?}]", e);
        });

    tokio::spawn(fut);
}
