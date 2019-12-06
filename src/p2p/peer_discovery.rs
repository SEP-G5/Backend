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

            let correct_from_addr = self.shuffle_node.unwrap() == from;
            if correct_from_addr {
                self.pending_resp.remove(&from);

                let req_accepted = peers.is_some();
                if req_accepted {

                    let has_peers = peers.is_some();
                    if has_peers {
                        let mut peers = peers.unwrap();
                        let hmap = block_on(network.get_state().lock());
                        let hmap = &hmap.b2n_tx;
                        peers.retain(|addr| {
                            !network.is_my_addr(addr) &&
                                hmap
                                .keys()
                                .filter(|haddr| *haddr != addr)
                                .collect::<Vec<&SocketAddr>>()
                                .len() == 0
                        });

                        peers.iter().for_each(|addr| {
                            if network.is_my_addr(&addr) {
                                return;
                            }
                            // TODO filter out nodes we already have
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
                    }

                    self.shuffle_node = None;
                    self.shuffle_nodes.clear();
                    self.timer = Instant::now();
                } else {
                    // shuffle req was denied, if there are others, try
                    // with them
                    self.shuffle_node = None;
                    println!("[PeerDisc:on_peer_shuffle_resp] shuffle req denied {:?}, {:?}", self.state, self.pending_resp);
                    if self.neighbor_nodes.len() > 1 {
                        self.state = PeerDiscState::Shuffle;
                    }
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
        if self.state == PeerDiscState::Wait && self.neighbor_nodes.len() > 0 {
            self.prepare_shuffle(network);
            packet = Packet::PeerShuffleResp(Some(self.shuffle_nodes.clone()));
            println!("[PeerDisc:on_peer_shuffle_req] sending resp Some");
            self.timer = Instant::now();
        } else {
            packet = Packet::PeerShuffleResp(None);
            println!("[PeerDisc:on_peer_shuffle_req] sending resp None");
        }

        network.unicast(packet, &from);
        self.print_neighbors();
    }

    fn print_nodes(nodes: &Vec<SocketAddr>) {
        nodes.iter()
            .for_each(|&addr| {
                println!("\t{}", addr);
            });
    }

    /// Network may drop and accepet new connections, independently of
    /// the peer discovery algorithm. Keep our list of nodes up to date
    /// with the networks.
    fn update_neighbors(&mut self, network: &Network) {
        let net_nodes: Vec<SocketAddr>;
        {
            let state = block_on(network.get_state().lock());
            net_nodes = state.b2n_tx.keys().cloned().collect();
        }

        let node_dc = self.neighbor_nodes.len() > net_nodes.len();
        let new_node = self.neighbor_nodes.len() < net_nodes.len();

        if node_dc {
            println!("\n[PeerDisc:update_neighbors] node dc");
            Self::print_nodes(&self.neighbor_nodes);
            println!("/|\\ peer disc,  \\|/ network");
            Self::print_nodes(&net_nodes);
            self.neighbor_nodes.retain(|&addr| {
                net_nodes.iter().filter(|&net_addr| {
                    addr == *net_addr
                }).collect::<Vec<&SocketAddr>>().len() == 1
            });
            println!("---- After ----");
            Self::print_nodes(&self.neighbor_nodes);
            println!("/|\\ peer disc,  \\|/ network");
            Self::print_nodes(&net_nodes);
            println!("");
        } else if new_node {
            println!("\n[PeerDisc:update_neighbors] new node");
            Self::print_nodes(&self.neighbor_nodes);
            println!("/|\\ peer disc,  \\|/ network");
            Self::print_nodes(&net_nodes);
            // find missing node, and add to our nodes
            net_nodes.iter()
                .for_each(|&net_addr| {
                    let o_addr = self.neighbor_nodes.iter().find(|&addr| {
                        *addr == net_addr
                    });
                    if o_addr.is_none() {
                        self.neighbor_nodes.push(net_addr.clone());
                    }
                });
            println!("---- After ----");
            Self::print_nodes(&self.neighbor_nodes);
            println!("/|\\ peer disc,  \\|/ network");
            Self::print_nodes(&net_nodes);
            println!("");
        } else {
            // check that we have the same nodes

        }
    }

    // TODO may block for long periods of time
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
                network.close_node_from_addr(&self.shuffle_node.unwrap());
                self.state = PeerDiscState::Shuffle;
                self.shuffle_node = None;
                return;
            } else {
                // TODO have this shuffle timer be random
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
            self.shuffle_nodes.iter().for_each(|addr| {
                network.close_node_from_addr(addr);
            });
            println!(
                "[PeerDisc:prepare_shuffle] closed {} nodes in\
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

        let packet = Packet::PeerShuffleReq(self.shuffle_nodes.clone());
        println!("[PeerDisc:state_shuffle] sending req");
        if let Err(e) = network.unicast(packet, &self.shuffle_node.unwrap()) {
            // cannot talk to node, remove it and redo shuffle with someone else

        }
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
            if network.is_my_addr(addr) {
                return;
            }
            let fut = network.add_node_from_addr(addr);
            let res = block_on(fut);
            match res {
                Ok(_) => {
                    println!("[PeerDisc:state_init] connected to [{}]", addr);
                    self.neighbor_nodes.push(*addr);
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
        self.timer = Instant::now();
    }

    fn print_neighbors(&self) {
        println!("~#~ Neighbor nodes:");
        self.neighbor_nodes
            .iter()
            .for_each(|addr| {
                println!("\t{}", addr);
            });
    }
}
