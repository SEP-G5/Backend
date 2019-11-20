pub mod block;
pub mod hash;
pub mod transaction;
pub mod util;

// ========================================================================== //

use block::Block;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::usize;
use transaction::Transaction;

// ========================================================================== //

#[derive(Debug)]
pub enum ChainErr {
    BadParent,
    NonUniqueTransactionID,
    BadTransaction(String),
}

// ========================================================================== //

/// Type of the blocks that are stored in the blockchain
type BlockType = Block<Transaction>;

// ========================================================================== //

/// A single node in the blockchain. Each node contains a list of possible
/// blocks at this position. The reason why multiple blocks are kept is to be
/// able to monitor the longest chains.
///
#[derive(Serialize, Deserialize)]
pub struct Node {
    /// List of potential blocks at each position in the chain. The index in the
    /// vector represents the position.
    blocks: Vec<BlockType>,
}

// ========================================================================== //

impl Node {
    /// Construct a new node without any blocks assigned to it
    ///
    pub fn new() -> Node {
        Node { blocks: vec![] }
    }

    /// Add a block to the node.
    ///
    pub fn add_block(&mut self, block: BlockType) {
        self.blocks.push(block)
    }

    /// Returns a reference to the vector of blocks stored at the node
    pub fn get_blocks(&self) -> &Vec<BlockType> {
        &self.blocks
    }
}

// ========================================================================== //

/// The main blockchain structure. This contains the list of nodes that makes up
/// the blockchain.
///
#[derive(Serialize, Deserialize)]
pub struct Chain {
    /// List of the nodes that make up the chain
    nodes: Vec<Node>,
}

// ========================================================================== //

impl Chain {
    /// Construct a new empty blockchain.
    pub fn new() -> Chain {
        let mut chain = Chain { nodes: vec![] };

        // Add the "genesis" block
        chain.nodes.push(Node::new());
        let (t, t_s) = Transaction::debug_make_register(format!("GENESIS_BIKE"));
        let b = Block::new(hash::EMPTY_HASH, t);
        chain.nodes[0].add_block(b);

        chain
    }

    /// Push a new block at the end of the blockchain.
    ///
    /// # Requirements
    /// Before the block can actually be pushed, some conditions must be met.
    /// The user will get an error back describing what happen, if an error
    /// occured.
    ///
    /// 1. The parent (previous) block for the one being pushed must match that
    ///    of the last block in the chain. Otherwise we are missing blocks or
    ///    the block is simply invalid.
    /// 2. All transactions in the block must be valid. This further means that:
    ///    - All transactions are correctly signed; and
    ///    - Inputs and outputs must be correct for the type of block
    ///      (register or transfer).
    ///
    /// # Longest chain
    /// The next concept is about longest chains. This check occurs if the block
    /// happens to refer to a parent (previous block hash) that is not actually
    /// the last block in the chain. We then need to determine, through asking
    /// other nodes, if the block is simply missing from our chain or if the
    /// block is invalid.
    ///
    ///
    pub fn push(&mut self, block: BlockType) -> Result<(), ChainErr> {
        // Find parent index
        let mut parent_index: usize = usize::MAX;
        for (i, node) in self.nodes.iter().enumerate() {
            for b in node.get_blocks().iter() {
                if b.calc_hash() == *block.get_parent_hash() {
                    parent_index = i;
                }
            }
        }
        if parent_index == usize::MAX {
            return Err(ChainErr::BadParent);
        }
        println!("Parent index: {}", parent_index);

        // Extend node list if necessary
        if parent_index >= self.nodes.len() - 1 {
            self.nodes.push(Node::new());
            println!("Added node to extend to length: {}", self.nodes.len());
        }

        // Add block
        self.nodes[parent_index + 1].add_block(block);

        Ok(())
    }

    pub fn block_count(&self) -> usize {
        self.nodes.iter().map(|n| n.get_blocks().len()).sum()
    }

    pub fn get_genesis_block(&self) -> &BlockType {
        &self.nodes[0].get_blocks()[0]
    }

    pub(crate) fn write_dot(&self, path: &str) {
        let mut dot = format!("digraph Blockchain {{\n");

        // Build graph
        let mut idx = 0;
        for (i, node) in self.nodes.iter().enumerate() {
            let idx_next = idx + node.get_blocks().len();
            for (j, b) in node.get_blocks().iter().enumerate() {
                let h = &hash::hash_to_str(&b.calc_hash())[0..6];
                let tid = b.get_data().get_id();
                dot.push_str(&format!(
                    "\t{}[label=\"{}...\\nID: {}\", shape=box];\n",
                    idx + j,
                    h,
                    tid
                ));
                // Handle children if available
                if i != self.nodes.len() - 1 {
                    for (k, b1) in self.nodes[i + 1].get_blocks().iter().enumerate() {
                        if *b1.get_parent_hash() == b.calc_hash() {
                            dot.push_str(&format!("\t{} -> {};\n", idx + j, idx_next + k));
                        }
                    }
                }
            }
            idx = idx_next;
        }

        // Write to file
        dot.push_str("}");
        fs::write(path, &dot).expect("Failed to write dot file");
    }
}

// ========================================================================== //
// Test
// ========================================================================== //

#[cfg(test)]
mod tests {
    use super::*;
    use rust_sodium::crypto::sign;

    #[test]
    fn test_create() {
        let chain = Chain::new();
        //assert_eq!(chain.len(), 0, "Newly created blockchains must be empty");
    }

    #[test]
    fn test_append() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, t1_s) = Transaction::debug_make_transfer(&t0, &t0_s);

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);

        chain.push(block_0).expect("Failed to add block 0:");
        chain.push(block_1).expect("Failed to add block 1:");

        assert_eq!(
            chain.block_count(),
            3,
            "Blockchain should contain 3 blocks (+1 for genesis)"
        );

        chain.write_dot("chain.dot");
    }

    /*
    #[test]
    #[should_panic]
    fn test_append_invalid_id() {
        let mut chain = Chain::new();

        // First transaction
        let (t0_p, t0_s) = sign::gen_keypair();
        let mut t0 = Transaction::new(format!("SN1337BIKE"), None, t0_p.as_ref().to_vec());
        t0.sign(&t0_s);

        // Second transaction
        let (t1_p, _) = sign::gen_keypair();
        let mut t1 = Transaction::new_debug(
            format!("MYCOOLBIKE"),
            util::make_timestamp(),
            Some(t0.get_public_key_output().clone()),
            t1_p.as_ref().to_vec(),
            Vec::new(),
        );
        t1.sign(&t0_s);

        // Blocks
        let block_0 = Block::new(hash::EMPTY_HASH, t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);

        chain.push(block_0).expect("Failed to add block 0:");
        chain.push(block_1).expect("Failed to add block 1:");
    }

    #[test]
    #[should_panic]
    fn test_append_dup_id() {
        let mut chain = Blockchain::new();

        // First transaction
        let (t0_p, t0_s) = sign::gen_keypair();
        let mut t0 = Transaction::new(format!("SN1337BIKE"), None, t0_p.as_ref().to_vec());
        t0.sign(&t0_s);

        // Second transaction
        let (t1_p, t1_s) = sign::gen_keypair();
        let mut t1 = Transaction::new(format!("SN1337BIKE"), None, t1_p.as_ref().to_vec());
        t1.sign(&t1_s);

        // Blocks
        let block_0 = Block::new(hash::EMPTY_HASH, t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);

        chain.push(block_0).expect("Failed to add block 0:");
        chain.push(block_1).expect("Failed to add block 1:");
    }
    */
}
