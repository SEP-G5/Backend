pub mod operation;

// ========================================================================== //

use crate::blockchain::{
    block::Block,
    transaction::{PubKey, Transaction},
    Chain,
};
use crate::rest::{
    self,
    server::{Peer, Peers},
};
use futures::{channel::oneshot, Future};
use operation::Operation;
use std::sync::mpsc;
use std::thread;

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
        backend.build_test_chain();
        backend
    }

    /// Run the backend.
    ///
    pub fn run(&self) {
        // Launch REST server
        let (rest_send, rest_recv) = mpsc::channel();
        thread::spawn(move || {
            rest::server::run_server(rest_send);
        });

        // Launch P2P communicator
        let (p2p_send, p2p_recv) = mpsc::channel::<Operation>();
        thread::spawn(move || {
            //p2p::comm::run(p2p_recv);
        });

        // Wait on messages
        loop {
            let res = rest_recv.try_recv();
            if let Ok(op) = res {
                match op {
                    Operation::QueryID {
                        id,
                        limit,
                        skip,
                        res,
                    } => {
                        /*println!(
                            "Got: \"QueryID {{ id: {}, limit: {}, skip: {} }}\"",
                            id, limit, skip
                        );*/
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
                        /*println!(
                            "Got: \"QueryPubKey {{ id: {}, limit: {}, skip: {} }}\"",
                            id, limit, skip
                        );*/
                        let blocks = self.chain.get_blocks_for_pub_key(&key);
                        let txs: Vec<Transaction> = blocks
                            .iter()
                            .skip(skip)
                            .take(limit)
                            .map(|b| b.get_data().clone())
                            .collect();
                        res.send(txs).expect("Failed to set \"QueryID\"result");
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
