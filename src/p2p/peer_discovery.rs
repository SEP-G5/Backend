use crate::p2p::network::Network;
use crate::p2p::packet::Packet;
use futures::executor::block_on;
use rand::seq::SliceRandom;
use rand::Rng;
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
        NodeInfo { addr, age: 0 }
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

    /// State change timer.
    timer: Instant,

    /// How long until we change our state.
    shuffle_timeout: Duration,

    shuffle_at_start: bool,
}

impl PeerDisc {
    pub fn new() -> PeerDisc {
        let mut rng = rand::thread_rng();
        PeerDisc {
            state: PeerDiscState::Init,
            neighbor_nodes: Vec::new(),
            pending_resp: HashMap::new(),
            shuffle_nodes: Vec::new(),
            shuffle_node: None,
            timer: Instant::now(),
            shuffle_timeout: Duration::from_secs(rng.gen_range(2, 7)),
            shuffle_at_start: false,
        }
    }

    /// Get a list of our current neighbors
    pub fn get_neighbors(&self) -> Vec<SocketAddr> {
        self.neighbor_nodes
            .iter()
            .map(|node| node.addr.clone())
            .collect()
    }

    /// @param from The addr we recieved the packet on.
    pub fn on_join_req(&self, port: u16, from: SocketAddr, network: &Network) {
        println!("[PeerDisc:on_join_req] port: {}, from: {}", port, from.ip());

        let mut joined_addr = from.clone();
        joined_addr.set_port(port);

        self.neighbor_nodes.iter().for_each(|node| {
            let packet = Packet::JoinFwd(joined_addr.clone());
            if let Err(e) = network.unicast(packet, &node.addr) {
                //TODO close the connection on error?
                println!(
                    "[PeerDisc:on_join_req] failed to unicast Packet::\
                     JoinFwd to {:?} with error [{:?}]",
                    node.addr, e
                );
            }
        });
    }

    /// Broadcast to our neighbors
    pub fn broadcast(&self, packet: Packet, network: &Network) {
        println!("broadcasting to {} neighbors", self.neighbor_nodes.len());
        self.neighbor_nodes.iter().for_each(|node| {
            if let Err(e) = network.unicast(packet.clone(), &node.addr) {
                println!(
                    "[PeerDisc:broadcast_neighbor] failed to broadcast to {} with error [{:?}]",
                    node.addr, e
                );
            }
        })
    }

    /// This is us, getting a recommendation to connect to @addr.
    /// TODO this could be a source of attack, sending false addrs.
    /// @param addr The addr provided in the JoinFwd packet.
    /// @param from The addr we recieved the packet on.
    pub fn on_join_fwd(&mut self, addr: SocketAddr, from: SocketAddr, network: &Network) {
        println!("[PeerDisc:on_join_fwd] addr: {}, from {}", addr, from);

        self.connect_to_addr(addr, network);
        self.print_neighbors();
    }

    fn reset_timer(&mut self) {
        self.timer = Instant::now();
        let mut rng = rand::thread_rng();
        self.shuffle_timeout = Duration::from_secs(rng.gen_range(4, 8));
    }

    /// We got a shuffle response
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
                        let mut peers: Vec<SocketAddr> = peers.unwrap();
                        Self::clean_addrs(&mut peers, from);
                        self.connect_to_addrs(peers, network);
                        // TODO maybe we close even if !has_peers?
                        let sffl_addr = self.shuffle_node.as_ref().unwrap().addr.clone();
                        self.neighbor_nodes.retain(|addr| addr.addr != sffl_addr);
                        network.close_node_from_addr(&sffl_addr);
                    }

                    self.shuffle_node = None;
                    self.shuffle_nodes.clear();
                    self.reset_timer();
                } else {
                    self.shuffle_node = None;
                    println!(
                        "[PeerDisc:on_peer_shuffle_resp] shuffle req denied {:?}, {:?}",
                        self.state, self.pending_resp
                    );

                    if self.neighbor_nodes.len() < 2 {
                        let nodes = self.shuffle_nodes.iter().map(|node| node.addr).collect();
                        self.connect_to_addrs(nodes, network);
                        self.shuffle_nodes.clear();
                        self.reset_timer();
                    } else {
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

    /// Example:
    ///
    ///   Given:
    ///     origin = 192.168.1.10
    ///     addrs = ["127.0.0.1:35011", "127.0.0.1:35012"]
    ///
    ///   Do:
    ///     127.0.0.1:35011 -> 192.168.1.10:35011
    ///     127.0.0.1:35012 -> 192.168.1.10:35012
    pub fn clean_addrs(addrs: &mut Vec<SocketAddr>, origin: SocketAddr) {
        let localhost: SocketAddr = "127.0.0.1:0".parse().expect("failed to parse socket addr");
        addrs.iter_mut().for_each(|addr| {
            if addr.ip() == localhost.ip() {
                let port = addr.port();
                *addr = origin;
                addr.set_port(port);
            }
        });
    }

    /// We got a shuffle request.
    pub fn on_peer_shuffle_req(
        &mut self,
        network: &Network,
        mut peers: Vec<SocketAddr>,
        from: SocketAddr,
    ) {
        println!("[PeerDisc:on_peer_shuffle_req] got req");
        let packet: Packet;
        if self.state == PeerDiscState::Wait {
            self.prepare_shuffle(network);
            let nodes: Vec<SocketAddr> = self
                .shuffle_nodes
                .iter()
                .map(|node| node.addr.clone())
                .collect();
            packet = Packet::PeerShuffleResp(Some(nodes));
            println!("[PeerDisc:on_peer_shuffle_req] sending resp Some");
            self.reset_timer();

            Self::clean_addrs(&mut peers, from);
            self.connect_to_addrs(peers, network);
        } else {
            packet = Packet::PeerShuffleResp(None);
            println!("[PeerDisc:on_peer_shuffle_req] denying shuffler req");
        }

        if let Err(e) = network.unicast(packet, &from) {
            println!("[PeerDisc] failed to unicast: [{:?}]", e);
        }
        self.shuffle_node = None;
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
                if self.timer.elapsed() > self.shuffle_timeout || self.shuffle_at_start {
                    if self.neighbor_nodes.len() == 0 && !self.shuffle_at_start {
                        println!("[PeerDisc:state_wait] no neighbor nodes, do init");
                        self.state = PeerDiscState::Init;
                        self.reset_timer();
                    } else {
                        println!("[PeerDisc:state_wait] shuffle");
                        self.state = PeerDiscState::Shuffle;
                        self.reset_timer();
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

        if self.neighbor_nodes.len() > 0 {
            self.shuffle_node = Some(self.neighbor_nodes[0].clone());
        }
        self.print_neighbors();
    }

    /// Abort the ongoing shuffle.
    /// Will drop the shuffle node
    fn abort_shuffle(&mut self, network: &Network) {
        if self.state != PeerDiscState::Shuffle {
            return;
        }
        self.shuffle_node = None;
        let tmp = self.shuffle_nodes.clone();
        for node_info in tmp.iter() {
            self.connect_to_addr(node_info.addr, network);
        }
        self.shuffle_nodes.clear();
        self.reset_timer();
        self.state = PeerDiscState::Wait;
    }

    fn state_shuffle(&mut self, network: &Network) {
        if self.neighbor_nodes.len() == 0 {
            println!("[PeerDisc:state_shuffle] no neighbor nodes, do init");
            self.state = PeerDiscState::Init;
            self.reset_timer();
            return;
        }

        self.prepare_shuffle(network);

        let mut nodes: Vec<SocketAddr> = self
            .shuffle_nodes
            .iter()
            .map(|node| node.addr.clone())
            .collect();

        let mut my_addr: SocketAddr = "127.0.0.1:0".parse().expect("failed to parse addr");
        my_addr.set_port(network.get_port());
        nodes.push(my_addr);
        let packet = Packet::PeerShuffleReq(nodes);
        println!("[PeerDisc:state_shuffle] sending req");
        let raddr = &self.shuffle_node.as_ref().unwrap().addr;
        if let Err(e) = network.unicast(packet, raddr) {
            println!(
                "[PeerDisc:state_shuffle] failed to talk with node {} with error [{:?}]",
                raddr, e
            );
            self.neighbor_nodes.retain(|addr| &addr.addr != raddr);
            self.abort_shuffle(network);
        } else {
            self.state = PeerDiscState::Wait;
            self.pending_resp.insert(
                self.shuffle_node.as_ref().unwrap().addr.clone(),
                Instant::now(),
            );
            self.reset_timer();
        }
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
                println!(
                    "[PeerDisc:state_init] failed to send JoinReq to {} \
                     with error [{:?}]",
                    node.addr, e
                );
            }
        });

        self.state = PeerDiscState::Wait;
        self.reset_timer();
    }

    fn print_neighbors(&self) {
        println!("~#~ Neighbor nodes ({}):", self.neighbor_nodes.len());
        let mut i = 0;
        for node in self.neighbor_nodes.iter() {
            i += 1;
            if i > 5 {
                println!("\t... ({} more)", self.neighbor_nodes.len() - 5);
                break;
            }
            println!("\t{}", node.addr);
        }
    }

    /// From an address, connect to it and make it a neighbor.
    fn connect_to_addr(&mut self, addr: SocketAddr, network: &Network) {
        if network.is_my_addr(&addr) {
            return;
        }
        let o_addr = self.neighbor_nodes.iter().find(|&node| node.addr == addr);
        if o_addr.is_none() {
            let fut = network.add_node_from_addr(&addr);
            let res = block_on(fut);
            if let Err(e) = res {
                println!(
                    "[PeerDisc:connect_to_addr] failed to connect to node \
                     {} with error [{:?}]",
                    addr, e
                );
            } else {
                self.neighbor_nodes.push(NodeInfo::new(addr.clone()));
            }
        }
    }

    /// Take a set of addresses, (nodes), connect to them and add them
    /// to our list of neighbor nodes.
    fn connect_to_addrs(&mut self, nodes: Vec<SocketAddr>, network: &Network) {
        nodes.iter().for_each(|&addr| {
            self.connect_to_addr(addr, network);
        });
    }
}
