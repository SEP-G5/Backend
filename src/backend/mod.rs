pub mod operation;

// ========================================================================== //

use crate::blockchain::{self, block::Block, transaction::Transaction, Chain, ChainErr};
use crate::p2p::network::Network;
use crate::p2p::packet::Packet;
use crate::p2p::peer_discovery::PeerDisc;
use crate::rest;
use crate::rest::server::{Peer, Peers};
use operation::Operation;
use rand::Rng;
use std::{
    collections::VecDeque,
    net::SocketAddr,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// ========================================================================== //

// Number of mining iterations per main-loop iteration
const MINE_ITER: u32 = 100;

// ========================================================================== //

#[derive(Debug)]
pub enum BackendErr {
    OpCancelled,
    ChainErr(ChainErr),
}

// ========================================================================== //

pub struct Backend {
    /// Blockchain
    chain: Chain,
    /// Transaction queue
    txs: VecDeque<Transaction>,
    /// Block being mined
    mined: Option<blockchain::BlockType>,
}

// ========================================================================== //

impl Backend {
    /// Create a backend object.
    ///
    pub fn new() -> Backend {
        Backend {
            chain: Chain::new(),
            txs: VecDeque::new(),
            mined: None,
        }
    }

    /// Run the backend.
    ///
    pub fn run(&mut self, net_addr: String, rest_port: u16) {
        // Launch REST server
        let (rest_send, rest_recv) = mpsc::channel();
        thread::spawn(move || {
            rest::server::run_server(rest_send, rest_port);
        });

        // Launch P2P communicator
        let mut network = Network::new(net_addr);

        let mut peer_disc = PeerDisc::new();

        // Do the initial blockchain setup
        self.initial_setup(&mut network);

        // Wait on messages
        loop {
            peer_disc.poll(&network);

            let res = network.try_recv();
            if let Some((packet, addr)) = res {
                self.handle_packet(&network, &mut peer_disc, packet, addr);
            }

            // Handle messages from REST server
            let res = rest_recv.try_recv();
            if let Ok(op) = res {
                match op {
                    Operation::QueryID {
                        id,
                        limit,
                        skip,
                        res,
                    } => {
                        let blocks = self.chain.get_blocks_for_id(&id);
                        let txs: Vec<Transaction> = blocks
                            .iter()
                            .skip(skip)
                            .take(limit)
                            .map(|b| b.get_data().clone())
                            .collect();
                        res.send(txs).expect("Failed to set \"QueryID\"result");
                    }
                    Operation::QueryPubKey {
                        key,
                        limit,
                        skip,
                        res,
                    } => {
                        let blocks = self.chain.get_blocks_for_pub_key(&key);
                        let txs: Vec<Transaction> = blocks
                            .iter()
                            .skip(skip)
                            .take(limit)
                            .map(|b| b.get_data().clone())
                            .collect();
                        res.send(txs).expect("Failed to set \"QueryID\"result");
                    }
                    Operation::QueryPeers { res } => {
                        println!("query peers TODO");
                        let neighbors = peer_disc.get_neighbors();
                        let peers = Peers {
                            peers: neighbors
                                .iter()
                                .map(|peer| Peer {
                                    ip: peer.to_string(),
                                })
                                .collect(),
                        };
                        res.send(peers).expect("failed to send QueryPeers");
                    }
                    Operation::CreateTransaction { transaction, res } => {
                        if let Err(e) = self.enqueue_tx(transaction.clone()) {
                            res.send(Err(e)).expect("Failed to send");
                        } else {
                            peer_disc.broadcast(Packet::PostTx(transaction), &network);
                            res.send(Ok(())).expect("Failed to send");
                        }
                    }
                    Operation::DebugDumpGraph => {
                        let mut rng = rand::thread_rng();
                        let num: u64 = rng.gen();
                        self.chain
                            .write_dot(&format!("chain_graph_{}", num))
                            .expect("Failed to dump graph");
                    }
                }
            }

            // Step the mining process once
            self.mine_step(&peer_disc, &network);

            //std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Run one step of the mining process
    fn mine_step(&mut self, peer_disc: &PeerDisc, network: &Network) {
        // Set currently mined block
        if self.txs.len() > 0 && self.mined.is_none() {
            let tx = self.txs.pop_front().unwrap();
            self.mined = Some(Block::new(self.chain.get_last_block().calc_hash(), tx));
        }

        // Mine block (at this point 'mined' cannot be 'None')
        let mut did_mine = false;
        if let Some(block) = &mut self.mined {
            for _ in [0..MINE_ITER].iter() {
                if let true = self.chain.mine_step(block) {
                    did_mine = true;
                }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        if did_mine {
            // Add block to blockchain and broadcast it
            let block = self.mined.take().expect("Mined cannot be 'None' here");
            println!("Successfully mined a block");
            let idx = self
                .chain
                .push(block.clone(), false)
                .expect("Failed to push block");
            peer_disc.broadcast(Packet::PostBlock(Some(block), idx), &network);
        }
    }

    /// Enqueue a transaction by first checking if it's valid
    fn enqueue_tx(&mut self, transaction: Transaction) -> Result<(), BackendErr> {
        // Check if the transaction is either queued or being mined
        // already
        let mut ignore = false;
        if let Some(mined) = &self.mined {
            if mined.get_data().get_signature() == transaction.get_signature() {
                ignore = true;
            }
        }
        ignore = ignore
            || self.txs.iter().fold(false, |acc, tx| {
                acc || tx.get_signature() == transaction.get_signature()
            });

        // If it's not, then enqueue it. And also broadcast it to other
        // nodes
        if !ignore {
            let block = Block::new(self.chain.get_last_block().calc_hash(), transaction.clone());
            if let Err(e) = self.chain.could_push(&block, true) {
                return Err(BackendErr::ChainErr(e));
            } else {
                self.txs.push_back(transaction);
                return Ok(());
            }
        }

        Ok(())
    }

    /// Handle a packet that was received from the network
    fn handle_packet(
        &mut self,
        network: &Network,
        peer_disc: &mut PeerDisc,
        packet: Packet,
        from: SocketAddr,
    ) {
        match packet {
            Packet::PostBlock(block, idx) => {
                if let Some(block) = block {
                    // Is this block valid to be placed in the blockchain?
                    if let Err(e) = self.chain.could_push(&block, false) {
                        println!(
                            "Received a block over the network that does not\
                             fit in our blockchain. Are we missing something?\
                             (e: {:?})",
                            e
                        );
                        panic!("Handle this by requesting the blocks from other nodes")
                    } else {
                        // Are we currently mining a block with the same
                        // transaction? In that case stop the mining and accept
                        // this block instead (provided it's valid).
                        if let Some(mined) = &self.mined {
                            if mined.get_data().get_signature() == block.get_data().get_signature()
                            {
                                //println!("Received a block that we ourselves are currently mining");
                                self.mined = None;
                            }
                        }

                        // Or are there a transaction queued up to be mined that
                        // matches the transaction?
                        self.txs
                            .retain(|tx| tx.get_signature() != block.get_data().get_signature());

                        // Where do we add this new block in the chain? Is the
                        // location (using parent hash) actually valid.
                        let at_idx = self
                            .chain
                            .push(block, false)
                            .expect("Failed to push block even though a pre-check was done");
                        assert_eq!(
                            idx, at_idx,
                            "Got block that was inserted correctly at the wrong index"
                        );
                    }
                }
            }
            Packet::GetBlock(idx) => {
                let longest_chain = self.chain.get_longest_chain();
                if let Some(t_blk) = longest_chain.get(idx as usize) {
                    let blk = Some((*t_blk).clone());
                    match network.unicast(Packet::PostBlock(blk, idx), &from) {
                        Ok(_) => {}
                        Err(_) => eprintln!("Error while unicasting packet to '{}'", from),
                    }
                } else {
                    println!("Another node asked for a packet which we do not have");
                }
            }
            Packet::PeerShuffleReq(peers) => peer_disc.on_peer_shuffle_req(network, peers, from),
            Packet::PeerShuffleResp(o_peers) => {
                peer_disc.on_peer_shuffle_resp(network, o_peers, from);
            }
            Packet::PostTx(transaction) => {
                if let Ok(_) = self.enqueue_tx(transaction.clone()) {
                    peer_disc.broadcast(Packet::PostTx(transaction), &network);
                }
            }
            Packet::JoinReq(node_port) => {
                peer_disc.on_join_req(node_port, from, &network);
            }
            Packet::JoinFwd(node_addr) => {
                peer_disc.on_join_fwd(node_addr, from, &network);
            }
            Packet::CloseConnection() => {
                eprintln!("got Packet::CloseConnection from network on backend");
            }
        }
    }

    fn initial_setup(&mut self, network: &mut Network) {
        // Simplication: Ask only a single node for the blocks. This keeps the
        // risk of receiving blocks from different longest chains down to a
        // minimum.

        // Are there any nodes we can select to target our requests at?
        // Otherwise we have no hope of getting a chain and can immediately
        // return
        if network.node_count() < 1 {
            println!(
                "No other nodes found, initial blockchain retrieval is\
                 therefore skipped"
            );
            return;
        }
        let target_addr = match network.get_node(0) {
            Some(addr) => addr,
            None => return,
        };

        // Current block to wait for
        let mut cur_idx = 0;
        const TIMEOUT: Duration = Duration::from_millis(500);
        'outer: loop {
            // Request the block
            if let Err(_) = network.unicast(Packet::GetBlock(cur_idx), &target_addr) {
                println!("Failed to send block request to node during initial setup");
                break 'outer;
            }
            let mut send_time = Instant::now();

            // Wait for block
            loop {
                // Resend if the operation timed out
                if send_time.elapsed() > TIMEOUT {
                    if let Err(_) = network.unicast(Packet::GetBlock(cur_idx), &target_addr) {
                        println!("Failed to send block request to node during initial setup");
                        break 'outer;
                    }
                    send_time = Instant::now();
                }

                let res = network.try_recv();
                if let Some((packet, addr)) = res {
                    if addr == target_addr {
                        match packet {
                            Packet::PostBlock(block, idx) => {
                                if idx == cur_idx {
                                    if let Some(block) = block {
                                        // Got block, add to chain and go on to the
                                        // next index.
                                        self.chain.push(block, false).expect(
                                            "Failed to push block when building initial chain",
                                        );
                                        cur_idx += 1;
                                    } else {
                                        // Index matched but there are not block for
                                        // it. This means we are done.
                                        break 'outer;
                                    }
                                }
                            }
                            Packet::GetBlock(idx) => {
                                match network.unicast(Packet::PostBlock(None, idx), &addr) {
                                    Ok(_) => {}
                                    Err(_) => {
                                        eprintln!("Error while unicasting packet to '{}'", addr)
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
