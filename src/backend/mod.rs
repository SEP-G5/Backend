pub mod operation;

// ========================================================================== //

use crate::blockchain::{
    self,
    block::Block,
    transaction::{PubKey, Transaction},
    Chain, ChainErr,
};
use crate::p2p::network::{self, Network};
use crate::p2p::packet::Packet;
use crate::p2p::peer_discovery::PeerDisc;
use crate::rest::{
    self,
    server::{Peer, Peers},
};
use futures::future::Future;
use futures::{channel::oneshot, executor::block_on};
use operation::Operation;
use rand::distributions::weighted::alias_method::Weight;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread;

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
        let mut backend = Backend {
            chain: Chain::new(),
            txs: VecDeque::new(),
            mined: None,
        };
        backend
    }

    /// Run the backend.
    ///
    pub fn run(&mut self, net_addr: String) {
        // Launch REST server
        let (rest_send, rest_recv) = mpsc::channel();
        thread::spawn(move || {
            rest::server::run_server(rest_send);
        });

        // Launch P2P communicator
        let mut network = Network::new(net_addr);

        let mut peer_disc = PeerDisc::new();

        // Wait on messages
        loop {
            peer_disc.poll(&network);

            let res = network.try_recv();
            if let Some(_op) = res {
                println!("got msg from p2p network");
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
                    Operation::QueryPeers { res: _ } => {
                        println!("query peers TODO");
                        //let packet = Packet::GetPeers;
                        //network.broadcast(packet);
                    }
                    Operation::CreateTransaction { transaction, res } => {
                        // Would the transaction be valid
                        let block = Block::new(
                            self.chain.get_last_block().calc_hash(),
                            transaction.clone(),
                        );
                        if let Err(e) = self.chain.could_push(&block, true) {
                            res.send(Err(BackendErr::ChainErr(e)))
                                .expect("Failed to send");
                        } else {
                            self.enqueue_tx(transaction.clone());
                            network.broadcast(Packet::PostTx(transaction));
                            res.send(Ok(())).expect("Failed to send");
                        }
                    }
                }
            }

            // Step the mining process once
            self.mine_step(&network);

            //std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Run one step of the mining process
    fn mine_step(&mut self, network: &Network) {
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
            self.chain
                .push(block.clone(), false)
                .expect("Failed to push block");
            network.broadcast(Packet::PostBlock(block));
        }
    }

    /// Enqueue a transaction by first checking if it's valid
    fn enqueue_tx(&mut self, transaction: Transaction) {
        self.txs.push_back(transaction);
    }
}
