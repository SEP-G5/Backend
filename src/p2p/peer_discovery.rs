use crate::p2p::network::Network;
use crate::p2p::packet::Packet;
use futures::executor::block_on;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(PartialEq)]
enum PeerDiscState {
    Init,
    Wait,
    Shuffle,
}

const STATIC_NODES: &[&str] = &[
    "127.0.0.1:35010",
    "127.0.0.1:35011",
    "127.0.0.1:35012",
    "127.0.0.1:35013",
    "127.0.0.1:35014",
    "127.0.0.1:35015",
];

pub struct PeerDisc {
    /// Our state
    state: PeerDiscState,

    /// Our neighbouring nodes. These are confirmed active nodes.
    neighbor_nodes: Vec<SocketAddr>,

    /// A node in this collection has been sent a request, but has not
    /// responded. Instant is when the request was sent.
    pending_resp: HashMap<SocketAddr, Instant>,

    /// Nodes that will be sent off to our shuffle node.
    /// Stored here in case it does not accept the shuffle.
    shuffle_nodes: Vec<SocketAddr>,

    /// The node we will do shuffle with.
    shuffle_node: Option<SocketAddr>,

    timer: Instant,
    shuffle_at_start: bool,
}

impl PeerDisc {
    pub fn new() -> PeerDisc {
        PeerDisc {
            state: PeerDiscState::Init,
            neighbor_nodes: Vec::new(),
            pending_resp: HashMap::new(),
            shuffle_nodes: Vec::new(),
            shuffle_node: None,
            timer: Instant::now(),
            shuffle_at_start: true,
        }
    }

    pub fn on_peer_shuffle_resp(
        &mut self,
        network: &Network,
        peers: Option<Vec<SocketAddr>>,
        from: SocketAddr,
    ) {
        if self.shuffle_node.is_some() {
            if self.shuffle_node.unwrap() == from {
                if peers.is_some() {
                    // shuffle req accpeted, connect to the given nodes
                    let peers = peers.unwrap();
                    peers.iter().for_each(|addr| {
                        let fut = network.add_node_from_addr(addr);
                        let res = block_on(fut);
                        if let Err(_) = res {
                            println!(
                                "[PeerDisc:on_peer_shuffle_resp] \
                                 failed to connect to node [{}]",
                                addr
                            );
                        }
                    });
                    self.shuffle_node = None;
                    self.shuffle_nodes.clear();
                    self.timer = Instant::now();
                } else {
                    // shuffle req was denied, try someone else
                    self.shuffle_node = None;
                    self.state = PeerDiscState::Shuffle;
                }
            } else {
                println!(
                    "[PeerDisc:on_peer_shuffle_resp] got resp, but \
                     from wrong node, expected [{}] but got [{}]",
                    self.shuffle_node.unwrap(),
                    from
                );
            }
        } else {
            println!("[PeerDisc:on_peer_shuffle_resp] got unexpected resp");
        }
    }

    pub fn on_peer_shuffle_req(&mut self, network: &Network, peers: Vec<SocketAddr>, from: SocketAddr) {
        let packet: Packet;
        if self.state == PeerDiscState::Wait {
            self.prepare_shuffle();
            packet = Packet::PeerShuffleResp(Some(self.shuffle_nodes.clone()));
            self.timer = Instant::now();
        } else {
            packet = Packet::PeerShuffleResp(None);
        }

        network.unicast(packet, &self.shuffle_node.unwrap());
        self.shuffle_node = None;
    }

    // TODO may block for long periods of time because of state_init()
    // which may block when connecting to nodes.
    // Can also block on on_peer_shuffle_resp
    pub fn poll(&mut self, network: &Network) {
        match self.state {
            PeerDiscState::Init => self.state_init(network),
            PeerDiscState::Wait => self.state_wait(network),
            PeerDiscState::Shuffle => self.state_shuffle(network),
        }
    }

    fn state_wait(&mut self, network: &Network) {
        if self.pending_resp.len() == 0 {

            if self.shuffle_node.is_some() {
                println!("[PeerDisc:state_wait] shuffle node timed out");
                network.close_node_from_addr(&self.shuffle_node.unwrap());
                self.state = PeerDiscState::Shuffle;
                self.shuffle_node = None;
                return;
            }
        }

        if self.shuffle_node.is_none() {
            // TODO have this shuffle timer be random
            const SHUFFLE_TIMER: Duration = Duration::from_secs(120);
            if self.timer.elapsed() > SHUFFLE_TIMER || self.shuffle_at_start {
                println!("[PeerDisc:state_wait] shuffle");
                self.shuffle_at_start = false;
                self.state = PeerDiscState::Shuffle;
                self.timer = Instant::now();
            }
        }

        self.pending_resp.retain(|&addr, time| {
            const TIMEOUT: Duration = Duration::from_secs(30);
            if time.elapsed() > TIMEOUT {
                println!("[PeerDisc:state_wait_shuffle] node {:?} timed out.", addr);
                false
            } else {
                true
            }
        });
    }

    /// Prepare the data required for a shuffle.
    fn prepare_shuffle(&mut self) {
        self.neighbor_nodes.shuffle(&mut rand::thread_rng());

        if self.shuffle_nodes.len() == 0 {
            self.shuffle_nodes = self.neighbor_nodes.split_off(self.neighbor_nodes.len() / 2);
        }

        self.shuffle_node = Some(self.neighbor_nodes[0].clone());
    }

    fn state_shuffle(&mut self, network: &Network) {
        if self.neighbor_nodes.len() == 1 {
            println!("[PeerDisc:state_shuffle] cannot shuffle with 1 node");
            self.state = PeerDiscState::Wait;
            return;
        } else if self.neighbor_nodes.len() == 0 {
            println!("[PeerDisc:state_shuffle] no neighbor nodes, canceling shuffle");
            self.state = PeerDiscState::Wait;
            self.timer = Instant::now();
            return;
        }

        self.prepare_shuffle();

        let packet = Packet::PeerShuffleReq(self.shuffle_nodes.clone());
        network.unicast(packet, &self.shuffle_node.unwrap());
        self.state = PeerDiscState::Wait;
        self.pending_resp
            .insert(self.shuffle_node.unwrap().clone(), Instant::now());
        self.timer = Instant::now();
    }

    fn state_init(&mut self, network: &Network) {
        println!("[PeerDisc:state_init] init");
        let static_peers: Vec<SocketAddr> = STATIC_NODES
            .iter()
            .map(|addr| {
                addr.parse::<SocketAddr>()
                    .expect("failed to parse network address")
            })
            .collect();

        const PEER_LIMIT: usize = 10;

        static_peers.iter().take(PEER_LIMIT).for_each(|addr| {
            let fut = network.add_node_from_addr(addr);
            let res = block_on(fut);
            match res {
                Ok(_) => {
                    self.pending_resp.insert(addr.clone(), Instant::now());
                }
                Err(_) => {
                    println!(
                        "Failed to connect to (hard coded) known \
                         node {}.",
                        addr
                    );
                }
            }
        });

        self.state = PeerDiscState::Wait;
        println!(
            "[PeerDisc:state_init] waiting for {} responses",
            self.pending_resp.len()
        );
        self.timer = Instant::now();
    }
}

/*

OBJECTIVE: Build active nodes list.

1. Peer discovery connects to x amount of static nodes.
   Then sends out getnodes requests to them.

   (1.1. Ask some DNS server for list of known nodes)

2. Backend will receive getnode responses, will forward them to me (on_get_nodes()).
   2.1 Recursively ask for nodes

3.

3. Start a timer, when timer is done, shuffle the connections, then reset timer.

*/
