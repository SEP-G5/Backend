use crate::p2p::network::Network;
use crate::p2p::packet::Packet;
use futures::executor::block_on;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[derive(PartialEq, Debug)]
enum PeerDiscState {
    Init,
    Wait,
    Shuffle,
}

const STATIC_NODES: &[&str] = &["127.0.0.1:35010", "127.0.0.1:35011", "127.0.0.1:35012"];

#[derive(Clone, Debug)]
struct NodeInfo {
    pub addr: SocketAddr,
    pub age: u64,
}

impl NodeInfo {
    fn new(addr: SocketAddr) -> NodeInfo {
        NodeInfo{
            addr,
            age: 0
        }
    }
}

pub struct PeerDisc {
    /// Our state
    state: PeerDiscState,

    /// Our neighboring nodes. These are confirmed active nodes.
    /// (address, age)
    neighbor_nodes: Vec<NodeInfo>,

    /// A node in this collection has been sent a request, but has not
    /// responded. Instant is when the request was sent.
    pending_resp: HashMap<SocketAddr, Instant>,

    /// Nodes that will be sent off to our shuffle node.
    /// Stored here in case it does not accept the shuffle.
    /// (address, age)
    shuffle_nodes: Vec<NodeInfo>,

    /// The node we will do shuffle with.
    shuffle_node: Option<NodeInfo>,

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

    /// @param from The addr we recieved the packet on.
    pub fn on_join_req(&self, port: u16, from: SocketAddr, network: &Network) {
        println!(
            "[PeerDisc:on_join_req] port: {}, from: {}",
            port,
            from.ip()
        );

        let mut joined_addr = from.clone();
        joined_addr.set_port(port);

        self.neighbor_nodes.iter().for_each(|node| {
            let packet = Packet::JoinFwd(joined_addr.clone());
            if let Err(e) = network.unicast(packet, &node.addr) {
                //TODO close the connection on error?
                println!("[PeerDisc:on_join_req] failed to unicast Packet::\
                          JoinFwd to {:?} with error [{:?}]", node.addr, e);
            }
        });
    }

    /// This is us, getting a recommendation to connect to @addr.
    /// TODO this could be a source of attack, sending false addrs.
    /// @param addr The addr provided in the JoinFwd packet.
    /// @param from The addr we recieved the packet on.
    pub fn on_join_fwd(&mut self, addr: SocketAddr, from: SocketAddr,
                       network: &Network) {
        println!("[PeerDisc:on_join_fwd] addr: {}, from {}", addr, from);

        self.connect_to_addr(addr, network);
        self.print_neighbors();
    }

    /// In response we see the result of the request.
    pub fn on_peer_shuffle_resp(
        &mut self,
        network: &Network,
        peers: Option<Vec<SocketAddr>>,
        from: SocketAddr,
    ) {
        println!("[PeerDisc:on_peer_shuffle_resp] got resp");

        let expecting_resp = self.shuffle_node.is_some();
        if expecting_resp {
            let correct_from_addr = self.shuffle_node.as_ref().unwrap().addr == from;
            if correct_from_addr {
                self.pending_resp.remove(&from);

                let req_accepted = peers.is_some();
                if req_accepted {
                    let has_peers = peers.is_some();
                    if has_peers {
                        let peers: Vec<SocketAddr> = peers.unwrap();
                        self.connect_to_addrs(peers, network);
                    }

                    self.shuffle_node = None;
                    self.shuffle_nodes.clear();
                    self.timer = Instant::now();
                } else {
                    // shuffle req was denied, if there are others, try
                    // with them
                    self.shuffle_node = None;
                    println!(
                        "[PeerDisc:on_peer_shuffle_resp] shuffle req denied {:?}, {:?}",
                        self.state, self.pending_resp
                    );
                    if self.neighbor_nodes.len() > 1 {
                        self.state = PeerDiscState::Shuffle;
                    }
                }
            } else {
                println!(
                    "[PeerDisc:on_peer_shuffle_resp] got resp, but \
                     from wrong node, expected [{}] but got [{}]",
                    self.shuffle_node.as_ref().unwrap().addr,
                    from
                );
            }
        } else {
            println!("[PeerDisc:on_peer_shuffle_resp] got unexpected resp");
        }
        self.print_neighbors();
    }

    /// In request we look at the data, and formulate a response.
    pub fn on_peer_shuffle_req(
        &mut self,
        network: &Network,
        peers: Vec<SocketAddr>,
        from: SocketAddr,
    ) {
        println!("[PeerDisc:on_peer_shuffle_req] got req");
        let packet: Packet;
        if self.state == PeerDiscState::Wait && self.neighbor_nodes.len() > 1 {
            self.prepare_shuffle(network);
            let nodes: Vec<SocketAddr> = self.shuffle_nodes.iter()
                .map(|node| {
                    node.addr.clone()
                }).collect();
            packet = Packet::PeerShuffleResp(Some(nodes));
            println!("[PeerDisc:on_peer_shuffle_req] sending resp Some");
            self.timer = Instant::now();
            self.connect_to_addrs(peers, network);
        } else {
            packet = Packet::PeerShuffleResp(None);
            println!("[PeerDisc:on_peer_shuffle_req] sending resp None");
        }

        if let Err(e) = network.unicast(packet, &from) {
            println!("[PeerDisc] failed to unicast: [{:?}]", e);
        }
        self.print_neighbors();
    }

    /// Remove dead neighbors, and update age of the living
    fn update_neighbors(&mut self, network: &Network) {
        let net_nodes: Vec<SocketAddr>;
        {
            let state = block_on(network.get_state().lock());
            net_nodes = state.b2n_tx.keys().cloned().collect();
        }

        let node_dc = self.neighbor_nodes.len() > net_nodes.len();
        if node_dc {
            println!("\n[PeerDisc:update_neighbors] node dc");
            self.neighbor_nodes.retain(|node| {
                net_nodes
                    .iter()
                    .filter(|&net_addr| node.addr == *net_addr)
                    .collect::<Vec<&SocketAddr>>()
                    .len()
                    == 1
            });
            self.print_neighbors();
        }

        self.neighbor_nodes.iter_mut().for_each(|node| {
            node.age += 1;
        });
    }

    // TODO may block for long periods of time, is it a problem?
    pub fn poll(&mut self, network: &Network) {
        self.update_neighbors(network);

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
                network.close_node_from_addr(&self.shuffle_node.as_ref().unwrap().addr);
                self.state = PeerDiscState::Shuffle;
                self.shuffle_node = None;
                return;
            } else {
                // TODO have this shuffle timer be random?
                const SHUFFLE_TIMER: Duration = Duration::from_secs(120);
                if self.timer.elapsed() > SHUFFLE_TIMER || self.shuffle_at_start {
                    if self.neighbor_nodes.len() == 0 && !self.shuffle_at_start {
                        println!("[PeerDisc:state_wait] no neighbor nodes, do init");
                        self.state = PeerDiscState::Init;
                        self.timer = Instant::now();
                    } else {
                        println!("[PeerDisc:state_wait] shuffle");
                        self.state = PeerDiscState::Shuffle;
                        self.timer = Instant::now();
                    }
                    self.shuffle_at_start = false;
                }
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
    /// self.shuffle_nodes will only be filled if it is cleared before calling.
    fn prepare_shuffle(&mut self, network: &Network) {
        // TODO instead of just shuffleing random, take the age of the node
        // into consideration.
        self.neighbor_nodes.shuffle(&mut rand::thread_rng());

        if self.shuffle_nodes.len() == 0 && self.neighbor_nodes.len() > 1 {
            self.shuffle_nodes = self.neighbor_nodes.split_off(self.neighbor_nodes.len() / 2);
            self.shuffle_nodes.iter().for_each(|node| {
                network.close_node_from_addr(&node.addr);
            });
            println!(
                "[PeerDisc:prepare_shuffle] closed {} nodes in \
                 shuffle",
                self.shuffle_nodes.len()
            );
        }

        self.shuffle_node = Some(self.neighbor_nodes[0].clone());
        self.print_neighbors();
    }

    fn state_shuffle(&mut self, network: &Network) {
        if self.neighbor_nodes.len() == 0 {
            println!("[PeerDisc:state_shuffle] no neighbor nodes, do init");
            self.state = PeerDiscState::Init;
            self.timer = Instant::now();
            return;
        }

        self.prepare_shuffle(network);

        let nodes: Vec<SocketAddr> = self.shuffle_nodes.iter()
            .map(|node| {
                node.addr.clone()
            }).collect();
        let packet = Packet::PeerShuffleReq(nodes);
        println!("[PeerDisc:state_shuffle] sending req");
        if let Err(e) = network.unicast(packet, &self.shuffle_node.as_ref().unwrap().addr) {
            // cannot talk to node, remove it and redo shuffle with someone else
        }
        self.state = PeerDiscState::Wait;
        self.pending_resp
            .insert(self.shuffle_node.as_ref().unwrap().addr.clone(), Instant::now());
        self.timer = Instant::now();
    }

    fn state_init(&mut self, network: &Network) {
        println!("[PeerDisc:state_init] init");
        let mut static_peers: Vec<SocketAddr> = STATIC_NODES
            .iter()
            .map(|addr| {
                addr.parse::<SocketAddr>()
                    .expect("failed to parse network address")
            })
            .filter(|addr| !network.is_my_addr(addr))
            .collect();
        static_peers.shuffle(&mut rand::thread_rng());

        const PEER_LIMIT: usize = 1;
        for addr in static_peers.iter() {
            if self.neighbor_nodes.len() >= PEER_LIMIT {
                break;
            }
            let fut = network.add_node_from_addr(addr);
            let res = block_on(fut);
            match res {
                Ok(_) => {
                    println!("[PeerDisc:state_init] connected to [{}]", addr);
                    self.neighbor_nodes.push(NodeInfo::new(addr.clone()));
                }
                Err(_) => {
                    println!(
                        "Failed to connect to (hard coded) known \
                         node {}.",
                        addr
                    );
                }
            }
        }

        while network.node_count() < self.neighbor_nodes.len() {
            std::thread::sleep(Duration::from_millis(1));
        }

        self.neighbor_nodes.iter().for_each(|node| {
            let packet = Packet::JoinReq(network.get_port());
            if let Err(e) = network.unicast(packet, &node.addr) {
                println!("[PeerDisc:state_init] failed to send JoinReq to {} \
                          with error [{:?}]", node.addr, e);
            }
        });

        self.state = PeerDiscState::Wait;
        self.timer = Instant::now();
    }

    fn print_neighbors(&self) {
        println!("~#~ Neighbor nodes:");
        self.neighbor_nodes.iter().for_each(|node| {
            println!("\t{}", node.addr);
        });
    }

    fn connect_to_addr(&mut self, addr: SocketAddr, network: &Network) {
        if network.is_my_addr(&addr) {
            return;
        }
        let o_addr = self.neighbor_nodes.iter()
            .find(|&node| node.addr == addr);
        if o_addr.is_none() {
            self.neighbor_nodes.push(NodeInfo::new(addr.clone()));
            let fut = network.add_node_from_addr(&addr);
            let res = block_on(fut);
            if let Err(e) = res {
                println!(
                    "[PeerDisc:connect_to_addr] failed to connect to node \
                     {} with error [{:?}]", addr, e);
            }
        }
    }

    /// Take a set of addresses, (nodes), add them to our list of
    /// neighbor nodes, and connect to them.
    fn connect_to_addrs(&mut self, nodes: Vec<SocketAddr>, network: &Network) {
        nodes.iter().for_each(|&addr| {
            self.connect_to_addr(addr, network);
        });
    }
}
