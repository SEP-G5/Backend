use crate::p2p::packet::Packet;
use crate::p2p::{node::Node, shared::Shared};
use futures::executor::block_on;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

pub type Tx = mpsc::Sender<(Packet, Option<SocketAddr>)>;
pub type Rx = mpsc::Receiver<(Packet, Option<SocketAddr>)>;

type StdRx = std::sync::mpsc::Receiver<(Packet, Option<SocketAddr>)>;
type StdTx = std::sync::mpsc::Sender<(Packet, Option<SocketAddr>)>;

pub enum NetError {
    NodeNotFound,
    Disconnected,
}

/// This is the gateway to the p2p network.
pub struct Network {
    state: Arc<Mutex<Shared>>,
    //n2b_rx: Rx,
    n2b_rx: StdRx,
    addr: SocketAddr,
}

impl Network {
    /// Create a new network object, and do setup
    /// @retval Rx The network-to-backend receive channel
    pub fn new(addr: String) -> Network {
        let (tx, rx) = mpsc::channel(1337);
        let (stdtx, stdrx) = std::sync::mpsc::channel();
        let shared = Shared::new(tx);
        let network = Network {
            state: Arc::new(Mutex::new(shared)),
            n2b_rx: stdrx,
            addr: addr
                .parse::<SocketAddr>()
                .expect("failed to parse network address"),
        };

        println!("Launching p2p server on {}.", addr);
        block_on(network.run(rx, stdtx));
        network
    }

    pub fn get_addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn is_my_addr(&self, other: &SocketAddr) -> bool {
        let localaddr: SocketAddr = format!("127.0.0.1:{}", self.get_addr().port())
            .parse::<SocketAddr>()
            .expect("failed to parse network address");
        *other == self.addr || *other == localaddr
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
    async fn broadcast_internal(&self, packet: Packet) {
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

    pub fn unicast(&self, packet: Packet, addr: &SocketAddr) -> Result<(), NetError> {
        block_on(self.unicast_internal(packet, addr))
    }

    pub async fn unicast_internal(
        &self,
        packet: Packet,
        addr: &SocketAddr,
    ) -> Result<(), NetError> {
        println!("unicasting packet");
        let nodes = &mut self.state.lock().await.b2n_tx;
        if let Some(tx) = nodes.get_mut(&addr) {
            match tx.send((packet.clone(), None)).await {
                Ok(_) => return Ok(()),
                Err(_) => return Err(NetError::Disconnected),
            }
        } else {
            return Err(NetError::NodeNotFound);
        }
    }

    /// Returns the current number of nodes that is one is connected to.
    pub fn node_count(&self) -> usize {
        block_on(self.node_count_internal())
    }

    pub async fn node_count_internal(&self) -> usize {
        let nodes = &mut self.state.lock().await.b2n_tx;
        nodes.len()
    }

    /// Returns the address of the node at the specified index. None if there is
    /// no node that matches the index.
    pub fn get_node(&self, index: usize) -> Option<SocketAddr> {
        block_on(self.get_node_internal(index))
    }

    pub async fn get_node_internal(&self, index: usize) -> Option<SocketAddr> {
        let nodes = &mut self.state.lock().await.b2n_tx;
        match nodes.iter_mut().nth(index) {
            Some((k, _)) => Some(*k),
            None => None,
        }
    }

    /// Disconnect the given node, close the connection.
    pub fn close_node_from_addr(&self, node_addr: &SocketAddr) {
        self.unicast(Packet::CloseConnection(), node_addr);
    }

    /// Take an open connection and run it as a node.
    /// @param stream An open connection, ready to be a node.
    pub fn add_node_from_stream(&self, stream: TcpStream) {
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut node = Node::new(stream, state).await;
            node.run().await;
        });
    }

    /// From a socket address, connect to it and run as a node.
    pub async fn add_node_from_addr(&self, node_addr: &SocketAddr) -> Result<(), ()> {
        let stream: TcpStream = match TcpStream::connect(node_addr).await {
            Ok(s) => s,
            Err(_) => return Err(()),
        };

        let state = self.state.clone();
        self.add_node_from_stream(stream);

        Ok(())
    }

    async fn run(&self, mut rx: Rx, stdtx: StdTx) {
        let mut listener = TcpListener::bind(&self.addr).await.expect("failed to bind");

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
