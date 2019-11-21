pub mod block;
pub mod hash;
pub mod transaction;
pub mod util;

// ========================================================================== //

use block::Block;
use serde::{Deserialize, Serialize};
use std::fs;
use std::usize;
use transaction::Transaction;

// ========================================================================== //

/// Enumeration of possible errors that can occur when working with the
/// blockchain.
///
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
    /// Construct a new blockchain containing only the "genesis" block.
    ///
    pub fn new() -> Chain {
        let mut chain = Chain { nodes: vec![] };

        // Add the "genesis" block
        chain.nodes.push(Node::new());
        let (t, _) = Transaction::debug_make_register(format!("GENESIS_BIKE"));
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
        // 1. Check valid signature (register and transfer)
        if let Err(e) = block.get_data().verify() {
            return Err(ChainErr::BadTransaction(e));
        }

        // 2. Check for unique id (register)
        if !block.get_data().has_input() {
            if !self.is_unique_id(&block) {
                return Err(ChainErr::NonUniqueTransactionID);
            }
        }

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

        // Check that the input to the transaction is valid
        if block.get_data().has_input() && !self.has_parent_block(&block) {
            return Err(ChainErr::BadTransaction(format!("Transaction input referes to a transaction that does not exist in any previous block")));
        }

        // Extend node list if necessary
        if parent_index >= self.nodes.len() - 1 {
            self.nodes.push(Node::new());
        }

        // Add block
        self.nodes[parent_index + 1].add_block(block);

        Ok(())
    }

    /// Function to determine if the ID of the transactions in a block are
    /// currently unique for the blockchain. If any existing blocks in the chain
    /// has the ID then this function returns false.
    fn is_unique_id(&self, block: &BlockType) -> bool {
        for n in self.nodes.iter() {
            for b in n.get_blocks().iter() {
                if b.get_data().get_id() == block.get_data().get_id() {
                    return false;
                }
            }
        }
        true
    }

    /// Returns whether or not there exists a block in the chain with the
    /// transction that the input of the transaction in the specified block
    /// refers to.
    ///
    fn has_parent_block(&self, block: &BlockType) -> bool {
        self.get_transaction_parent_block(block).is_some()
    }

    /// Returns the total number of blocks in the chain. This is not the longest
    /// part of the chain but rather the total number including any diverging
    /// blocks.
    ///
    pub fn block_count(&self) -> usize {
        self.nodes.iter().map(|n| n.get_blocks().len()).sum()
    }

    /// Returns the first (genesis) block in the blockchain.
    ///
    pub fn get_genesis_block(&self) -> &BlockType {
        &self.nodes[0].get_blocks()[0]
    }

    /// Returns the last block that is associated with a transaction that has
    /// the specified ID
    pub fn get_block_for_id(&self, id: &str) -> Option<&BlockType> {
        /*for node in self.nodes.iter().rev() {
            for block in node.get_blocks().iter() {}
        }*/
        None
    }

    /// Debug function that writes the entire blockchain structure to a .dot
    /// graph file. This graph can then be visualized with graphviz.
    ///
    pub fn write_dot(&self, path: &str) {
        let mut dot = format!("digraph Blockchain {{\n");

        for (i, node) in self.nodes.iter().enumerate() {
            for blk in node.get_blocks().iter() {
                let hash = blk.calc_hash();
                let hash_str = hash::hash_to_str(&hash);

                // Block itself
                dot.push_str(&format!(
                    "\t\"{}\"[label=\"{}...\\nID: {}\", shape=box];\n",
                    hash_str,
                    &hash_str[0..6],
                    blk.get_data().get_id()
                ));

                // Direct block chaining
                if i != self.nodes.len() - 1 {
                    for blk_c in self.nodes[i + 1].get_blocks().iter() {
                        let hash_str_c = hash::hash_to_str(&blk_c.calc_hash());
                        if *blk_c.get_parent_hash() == hash {
                            dot.push_str(&format!("\t\"{}\" -> \"{}\";\n", hash_str, hash_str_c));
                        }
                    }
                }

                // Transaction relations
                if let Some(blk_p) = self.get_transaction_parent_block(blk) {
                    let hash_str_p = hash::hash_to_str(&blk_p.calc_hash());
                    dot.push_str(&format!(
                        "\t\"{}\" -> \"{}\" [color=red, style=dotted];\n",
                        hash_str_p, hash_str
                    ));
                }
            }
        }

        // Write to file
        dot.push_str("}");
        fs::write(path, &dot).expect("Failed to write dot file");
    }

    /// Returns a reference to the block that the input of the transaction in
    /// the specified block refers to. If not block is found then None is
    /// returned instead.
    ///
    fn get_transaction_parent_block(&self, block: &BlockType) -> Option<&BlockType> {
        // Register blocks don't have any blocks they refer to
        if !block.get_data().has_input() {
            return None;
        }

        // Find the block that the transaction of this block refers to
        for n in self.nodes.iter().rev() {
            for b in n.get_blocks().iter() {
                let p_in = block
                    .get_data()
                    .get_public_key_input()
                    .as_ref()
                    .expect("Transaction representing transfer is expected to have an input");
                if b.get_data().get_public_key_output() == p_in {
                    return Some(&b);
                }
            }
        }
        None
    }
}

// ========================================================================== //
// Test
// ========================================================================== //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let chain = Chain::new();
        assert_eq!(
            chain.block_count(),
            1,
            "Newly created blockchains should only contain the genesis block"
        );
    }

    #[test]
    fn test_append() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, _) = Transaction::debug_make_transfer(&t0, &t0_s);
        let (t2, _) = Transaction::debug_make_register(format!("MYCOOLBIKE"));

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t2);
        let block_2 = Block::new(block_1.calc_hash(), t1);

        chain.push(block_0).expect("Chain::push failure (0)");
        chain.push(block_1).expect("Chain::push failure (1)");
        chain.push(block_2).expect("Chain::push failure (2)");

        assert_eq!(
            chain.block_count(),
            4,
            "Blockchain should contain 4 blocks (+1 for genesis)"
        );
    }

    #[test]
    #[should_panic(expected = "Chain::push failure (1): BadParent")]
    fn test_invalid_parent() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, _) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, _) = Transaction::debug_make_register(format!("MYCOOLBIKE"));

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(hash::EMPTY_HASH, t1);

        chain.push(block_0).expect("Chain::push failure (0)");
        chain.push(block_1).expect("Chain::push failure (1)");
    }

    /// Test to see that duplicate IDs of transactions are not allowed.
    #[test]
    #[should_panic(expected = "Chain::push failure (1): NonUniqueTransactionID")]
    fn test_append_dup_id() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, _) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, _) = Transaction::debug_make_register(format!("SN1337BIKE"));

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);

        chain.push(block_0).expect("Chain::push failure (0)");
        chain.push(block_1).expect("Chain::push failure (1)");
    }

    /// Test to see that a tampered transaction produces a verification error.
    #[test]
    #[should_panic(
        expected = "Chain::push failure: BadTransaction(\"content does not match the signature\")"
    )]
    fn test_invalid_transaction() {
        let mut chain = Chain::new();

        // Transactions
        let (mut t0, _) = Transaction::debug_make_register(format!("SN1337BIKE"));
        t0.set_id("MYCOOLBIKE");

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);

        chain.push(block_0).expect("Chain::push failure");
    }

    /// Test divering chains
    ///
    #[test]
    fn test_diverging() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, _) = Transaction::debug_make_register(format!("MYCOOLBIKE"));
        let (t2, _) = Transaction::debug_make_transfer(&t0, &t0_s);
        let (t3, _) = Transaction::debug_make_transfer(&t0, &t0_s);

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);
        let block_2 = Block::new(block_0.calc_hash(), t2);
        let block_3 = Block::new(block_1.calc_hash(), t3);

        chain.push(block_0).expect("Chain::push failure (0)");
        chain.push(block_1).expect("Chain::push failure (1)");
        chain.push(block_2).expect("Chain::push failure (2)");
        chain.push(block_3).expect("Chain::push failure (3)");
    }
}
