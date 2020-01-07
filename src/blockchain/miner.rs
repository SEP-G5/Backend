use crate::blockchain::{block::Block, transaction::Transaction, BlockType, Chain, ChainErr};
use std::collections::VecDeque;

// ========================================================================== //

// Number of hashes to try per mining operation
const TRIES: u32 = 1;

// ========================================================================== //

pub enum PushResult {
    Invalid(ChainErr),
    MissingParent,
    Added,
    Ignored,
}

// ========================================================================== //

pub struct Miner {
    /// Transaction queue
    queue: VecDeque<Transaction>,
    /// Block being mined
    mined: Option<BlockType>,
}

impl Miner {
    ///
    pub fn new() -> Miner {
        Miner {
            queue: VecDeque::new(),
            mined: None,
        }
    }

    pub fn push_transaction(&mut self, chain: &Chain, transaction: Transaction) -> PushResult {
        println!(
            "[PUSH TX] Checking to push transaction: '{}'",
            transaction.get_id()
        );

        // Check if the transaction is either queued or being mined
        // already
        let mut ignore = false;
        if let Some(mined) = &self.mined {
            if mined.get_data().get_signature() == transaction.get_signature() {
                ignore = true;
            }
        }
        ignore = ignore
            || self.queue.iter().fold(false, |acc, tx| {
                acc || tx.get_signature() == transaction.get_signature()
            });

        // If it's not, then enqueue it. And also broadcast it to other
        // nodes
        if !ignore {
            let block = Block::new(chain.get_last_block().calc_hash(), transaction.clone());
            if let Err(e) = chain.could_push(&block, true) {
                match e {
                    ChainErr::BadParent => {
                        // TODO(Filip Björklund): Deperecate use of this. SHOULD not happen
                        return PushResult::MissingParent;
                    }
                    _ => {
                        return PushResult::Invalid(e);
                    }
                }
            } else {
                println!("[PUSH TX] Pushed transaction: '{}'", transaction.get_id());

                self.queue.push_back(transaction);
                return PushResult::Added;
            }
        }

        // Transaction is already in the queue
        return PushResult::Ignored;
    }

    ///
    pub fn mine(&mut self, chain: &Chain) -> Option<BlockType> {
        // Set currently mined block
        if self.queue.len() > 0 && self.mined.is_none() {
            let tx = self.queue.pop_front().unwrap();
            self.mined = Some(Block::new(chain.get_last_block().calc_hash(), tx));
        }

        // Mine block (at this point 'mined' cannot be 'None')
        let mut did_mine = false;
        if let Some(block) = &mut self.mined {
            for _ in [0..TRIES].iter() {
                if let true = chain.mine_step(block) {
                    did_mine = true;
                }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        if did_mine {
            // Add block to blockchain and broadcast it
            let block = self.mined.take().expect("Mined cannot be 'None' here");
            return Some(block);
        }

        None
    }

    pub fn on_block_recv(&mut self, block: &BlockType) {
        // Check currently mined block
        let reset = match &self.mined {
            Some(b) => b.get_data() == block.get_data(),
            None => false,
        };
        if reset {
            self.mined = None;
        }

        // Check queue
        self.queue.retain(|tx| tx != block.get_data());
    }
}
