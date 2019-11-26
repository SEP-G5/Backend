pub mod operation;

// ========================================================================== //

use crate::blockchain::{
    block::Block,
    transaction::{PubKey, Transaction},
    Chain, ChainErr,
};
use crate::p2p::network::{self, Network};
use crate::rest::{
    self,
    server::{Peer, Peers},
};
use futures::{sync::oneshot};
use operation::Operation;
use std::sync::mpsc;
use std::thread;

// ========================================================================== //

#[derive(Debug)]
pub enum BackendErr {
    OpCancelled,
    ChainErr(ChainErr),
}

// ========================================================================== //

pub struct Backend {
    chain: Chain,
}

// ========================================================================== //

// ========================================================================== //

impl Backend {
    /// Create a backend object.
    ///
    pub fn new() -> Backend {
        let mut backend = Backend {
            chain: Chain::new(),
        };
        backend
    }

    /// Run the backend.
    ///
    pub fn run(&mut self) {
        // Launch REST server
        let (rest_send, rest_recv) = mpsc::channel();
        thread::spawn(move || {
            rest::server::run_server(rest_send);
        });

        // Launch P2P communicator
        let (network, network_recv) = Network::new();

        // Wait on messages
        loop {
            let res = network_recv.try_recv();
            if let Ok(_op) = res {
                println!("operation on network recv");
            }

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
                    Operation::QueryPeers { res: _ } => {}
                    Operation::CreateTransaction { transaction, res } => {
                        let longest_chain = self.chain.get_longest_chain();
                        let last_block = longest_chain.last().unwrap();
                        let b = Block::new(last_block.calc_hash(), transaction);
                        let msg = self.chain.push(b).or_else(|c| Err(BackendErr::ChainErr(c)));
                        res.send(msg).expect("Failed to set error code");
                        self.chain
                            .write_dot("graph.dot")
                            .expect("Failed to write dot");
                    }
                    _ => println!("Got some other op"),
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn build_test_chain(&mut self) {
        // Transactions
        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, _) = Transaction::debug_make_register(format!("MYCOOLBIKE"));
        let (t2, t2_s) = Transaction::debug_make_transfer(&t0, &t0_s);
        let (t3, _) = Transaction::debug_make_transfer(&t2, &t2_s);

        // Blocks
        let block_0 = Block::new(self.chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);
        let block_2 = Block::new(block_1.calc_hash(), t2);
        let block_3 = Block::new(block_2.calc_hash(), t3);

        self.chain.push(block_0).expect("Chain::push failure (0)");
        self.chain.push(block_1).expect("Chain::push failure (1)");
        self.chain.push(block_2).expect("Chain::push failure (2)");
        self.chain.push(block_3).expect("Chain::push failure (3)");
    }
}
