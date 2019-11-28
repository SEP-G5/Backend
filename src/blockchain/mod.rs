pub mod block;
pub mod hash;
pub mod transaction;
pub mod util;

// ========================================================================== //

use block::Block;
use hash::Hash;
use serde::{Deserialize, Serialize};
use std::{fs, io, usize};
use transaction::{PubKey, Transaction};

// ========================================================================== //

// Transactions relation color (for debug graph)
const TX_RELATION_COLOR: &str = "blue";

// Hash end pattern
const HASH_END_PATTERN: &str = "000";

// ========================================================================== //

/// Enumeration of possible errors that can occur when working with the
/// blockchain.
///
#[derive(Debug)]
pub enum ChainErr {
    BadParent,
    NonUniqueTransactionID,
    BadTransaction(String),
    BadHash,
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

    /// Returns the block that matches the specified hash from the node. If no
    /// block has the matching hash then "None" is returned instead
    pub fn get_block_from_hash(&self, hash: &Hash) -> Option<&BlockType> {
        for block in self.blocks.iter() {
            if &block.calc_hash() == hash {
                return Some(&block);
            }
        }
        None
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
    pub fn new() -> Chain {
        let mut chain = Chain { nodes: vec![] };

        // Add the "genesis" block
        chain.nodes.push(Node::new());
        let (t, _) = Transaction::debug_make_register(format!("GENESIS_BIKE"));
        let b = Block::new(hash::EMPTY_HASH, t);
        chain.nodes[0].add_block(b);

        chain
    }

    /// Push a new block at the end of the blockchain
    pub fn push(&mut self, block: BlockType) -> Result<(), ChainErr> {
        // Check that the block has a valid hash
        let hash_str = hash::hash_to_str(&block.calc_hash());
        if !hash_str.ends_with(HASH_END_PATTERN) {
            return Err(ChainErr::BadHash);
        }

        self.push_ignore_hash(block)
    }

    /// Push a new block at the end of the blockchain, ignoring the hash check.
    pub fn push_ignore_hash(&mut self, block: BlockType) -> Result<(), ChainErr> {
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
        let parent_index = match self.get_node_index_for_hash(&block.get_parent_hash()) {
            Some(i) => i,
            None => return Err(ChainErr::BadParent),
        };

        // Check that the input to the transaction is valid
        if let Some(input) = block.get_data().get_public_key_input() {
            let parent_chain = self.get_chain_for_block_hash(block.get_parent_hash());
            for blk in parent_chain.iter().rev() {
                if blk.get_data().get_id() == block.get_data().get_id() {
                    if blk.get_data().get_public_key_output() != input {
                        return Err(ChainErr::BadTransaction(format!(
                            "Transaction input referes to a transaction that \
                             is not the latest"
                        )));
                    } else {
                        break;
                    }
                }
            }

            if !self.has_parent_block(&block) {
                return Err(ChainErr::BadTransaction(format!(
                    "Transaction input refers to a transaction that does not \
                     exist in any previous block"
                )));
            }
        }

        // Extend node list if necessary
        if parent_index >= self.nodes.len() - 1 {
            self.nodes.push(Node::new());
        }
        self.nodes[parent_index + 1].add_block(block);
        Ok(())
    }

    /// Mine a block
    pub fn mine(&mut self, transaction: Transaction) -> BlockType {
        // Generate block
        let last_block = self.get_last_block();
        let mut block = Block::new(last_block.calc_hash(), transaction);
        loop {
            let hash = block.calc_hash();
            let hash_str = hash::hash_to_str(&hash);
            if hash_str.ends_with(HASH_END_PATTERN) {
                break;
            }
            block.inc_nonce();
        }
        block
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

    /// This function returns whether or not there exists a block in the chain
    /// for the parent of the specified block that contains a transaction with
    /// the transaction output key that matches the input key of the transaction
    /// in the specified block.
    fn has_parent_block(&self, block: &BlockType) -> bool {
        let input_key = match block.get_data().get_public_key_input() {
            Some(k) => k,
            None => return false,
        };

        let chain = self.get_chain_for_block_hash(block.get_parent_hash());
        for blk in chain.iter().rev() {
            if blk.get_data().get_public_key_output() == input_key {
                return true;
            }
        }

        false
    }

    /// Returns the total number of blocks in the chain. This is not the longest
    /// part of the chain but rather the total number including any diverging
    /// blocks.
    pub fn block_count(&self) -> usize {
        self.nodes.iter().map(|n| n.get_blocks().len()).sum()
    }

    /// Returns the first (genesis) block in the blockchain.
    pub fn get_genesis_block(&self) -> &BlockType {
        &self.nodes[0].get_blocks()[0]
    }

    /// Returns the last block in the longest chain of the blockchain.
    pub fn get_last_block(&self) -> &BlockType {
        let longest_chain = self.get_longest_chain();
        longest_chain.last().unwrap()
    }

    /// Returns a list of all blocks that are associated with the specified ID
    /// (only blocks in the longest chain are considered)
    pub fn get_blocks_for_id(&self, id: &str) -> Vec<&BlockType> {
        self.get_longest_chain()
            .into_iter()
            .filter(|block| block.get_data().get_id() == id)
            .collect()
    }

    /// Returns a list of blocks that have transactions where either the input
    /// or output matches the specified public key.
    /// (Only blocks in the longest chain are considered).
    pub fn get_blocks_for_pub_key(&self, key: &PubKey) -> Vec<&BlockType> {
        self.get_longest_chain()
            .into_iter()
            .filter(|block| {
                if let Some(inp) = block.get_data().get_public_key_input() {
                    if inp == key {
                        return true;
                    }
                }
                if block.get_data().get_public_key_output() == key {
                    return true;
                }
                false
            })
            .collect()
    }

    /// Debug function that writes the entire blockchain structure to a .dot
    /// graph file.
    ///
    /// This graph can then be visualized with graphviz.
    ///
    pub fn write_dot(&self, path: &str) -> io::Result<()> {
        self.write_dot_id(path, "")
    }

    /// Debug function that writes the entire blockchain structure to a .dot
    /// graph file. Each node that has a transaction matching the specified ID
    /// will also be colored red.
    ///
    /// This graph can then be visualized with graphviz.
    ///
    pub fn write_dot_id(&self, path: &str, id: &str) -> io::Result<()> {
        let mut dot = format!("digraph Blockchain {{\n");

        for (i, node) in self.nodes.iter().enumerate() {
            for blk in node.get_blocks().iter() {
                let hash = blk.calc_hash();
                let hash_str = hash::hash_to_str(&hash);

                let tx_paren = self.get_transaction_parent_block(blk);

                // Block itself
                let color = if !id.is_empty() && blk.get_data().get_id() == id {
                    "red"
                } else if tx_paren.is_none() {
                    TX_RELATION_COLOR
                } else {
                    "black"
                };

                dot.push_str(&format!(
                    "\t\"{}\"[label=\"{}...\\nID: {}\", shape=box, color={}];\n",
                    hash_str,
                    &hash_str[0..6],
                    blk.get_data().get_id(),
                    color
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
                if let Some(blk_p) = tx_paren {
                    let hash_str_p = hash::hash_to_str(&blk_p.calc_hash());
                    dot.push_str(&format!(
                        "\t\"{}\" -> \"{}\" [color={}, style=dotted];\n",
                        hash_str_p, hash_str, TX_RELATION_COLOR
                    ));
                }
            }
        }

        // Write to file
        dot.push_str("}");
        fs::write(path, &dot)
    }

    /// Returns the entire chain of blocks for the specified block hash.
    ///
    /// The returned chain is the direct blockchain and not the transaction
    /// relation chain
    fn get_chain_for_block_hash(&self, hash: &Hash) -> Vec<&BlockType> {
        let mut output = vec![];
        let mut hash = hash.clone();

        for node in self.nodes.iter().rev() {
            if let Some(block) = node.get_block_from_hash(&hash) {
                output.push(block);
                hash = block.get_parent_hash().clone();
            }
        }

        output.reverse();
        output
    }

    /// Returns a list of blocks that make up the longest chain.
    pub fn get_longest_chain(&self) -> Vec<&BlockType> {
        if let Some(node) = self.nodes.last() {
            if let Some(block) = node.get_blocks().first() {
                return self.get_chain_for_block_hash(&block.calc_hash());
            }
        }
        vec![]
    }

    /// Returns a reference to the block that the input of the transaction in
    /// the specified block refers to. If not block is found then None is
    /// returned instead.
    ///
    fn get_transaction_parent_block(&self, block: &BlockType) -> Option<&BlockType> {
        let input = match block.get_data().get_public_key_input() {
            Some(k) => k,
            None => return None,
        };
        let chain = self.get_chain_for_block_hash(block.get_parent_hash());
        for blk in chain.iter().rev() {
            if blk.get_data().get_public_key_output() == input {
                return Some(blk);
            }
        }
        None
    }

    /// Returns the index of the node that contains the block with the specified
    /// hash.
    fn get_node_index_for_hash(&self, hash: &Hash) -> Option<usize> {
        for (i, node) in self.nodes.iter().enumerate().rev() {
            if let Some(_) = node.get_block_from_hash(hash) {
                return Some(i);
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
        let (t1, _) = Transaction::debug_make_register(format!("MYCOOLBIKE"));
        let (t2, _) = Transaction::debug_make_transfer(&t0, &t0_s);

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);
        let block_2 = Block::new(block_1.calc_hash(), t2);

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure (0)");
        chain
            .push_ignore_hash(block_1)
            .expect("Chain::push failure (1)");
        chain
            .push_ignore_hash(block_2)
            .expect("Chain::push failure (2)");

        assert_eq!(
            chain.block_count(),
            4,
            "Blockchain should contain 4 blocks (+1 for genesis)"
        );
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

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure (0)");
        chain
            .push_ignore_hash(block_1)
            .expect("Chain::push failure (1)");
        chain
            .push_ignore_hash(block_2)
            .expect("Chain::push failure (2)");
        chain
            .push_ignore_hash(block_3)
            .expect("Chain::push failure (3)");
    }

    #[test]
    fn test_complex_chain() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, t1_s) = Transaction::debug_make_transfer(&t0, &t0_s);
        let (t2, t2_s) = Transaction::debug_make_register(format!("MYCOOLBIKE"));
        let (t3, t3_s) = Transaction::debug_make_transfer(&t2, &t2_s);
        let (t4, t4_s) = Transaction::debug_make_register(format!("WOW_BIKE_WOW"));
        let (t5, t5_s) = Transaction::debug_make_transfer(&t4, &t4_s);
        let (t6, t6_s) = Transaction::debug_make_transfer(&t1, &t1_s);
        let (t7, _) = Transaction::debug_make_transfer(&t5, &t5_s);
        let (t8, _) = Transaction::debug_make_transfer(&t3, &t3_s);
        let (t9, t9_s) = Transaction::debug_make_transfer(&t6, &t6_s);
        let (t10, _) = Transaction::debug_make_transfer(&t9, &t9_s);

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);
        let block_2 = Block::new(block_1.calc_hash(), t2);
        let block_3 = Block::new(block_2.calc_hash(), t3);
        let block_4 = Block::new(block_3.calc_hash(), t4);
        let block_5 = Block::new(block_4.calc_hash(), t5);
        let block_6 = Block::new(block_5.calc_hash(), t6);
        let block_7 = Block::new(block_6.calc_hash(), t7);
        let block_8 = Block::new(block_7.calc_hash(), t8);
        let block_9 = Block::new(block_8.calc_hash(), t9);
        let block_10 = Block::new(block_9.calc_hash(), t10);

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure (0)");
        chain
            .push_ignore_hash(block_1)
            .expect("Chain::push failure (1)");
        chain
            .push_ignore_hash(block_2)
            .expect("Chain::push failure (2)");
        chain
            .push_ignore_hash(block_3)
            .expect("Chain::push failure (3)");
        chain
            .push_ignore_hash(block_4)
            .expect("Chain::push failure (4)");
        chain
            .push_ignore_hash(block_5)
            .expect("Chain::push failure (5)");
        chain
            .push_ignore_hash(block_6)
            .expect("Chain::push failure (6)");
        chain
            .push_ignore_hash(block_7)
            .expect("Chain::push failure (7)");
        chain
            .push_ignore_hash(block_8)
            .expect("Chain::push failure (8)");
        chain
            .push_ignore_hash(block_9)
            .expect("Chain::push failure (9)");
        chain
            .push_ignore_hash(block_10)
            .expect("Chain::push failure (10)");
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

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure (0)");
        chain
            .push_ignore_hash(block_1)
            .expect("Chain::push failure (1)");
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

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure (0)");
        chain
            .push_ignore_hash(block_1)
            .expect("Chain::push failure (1)");
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

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure");
    }

    #[test]
    fn test_query_id() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, _) = Transaction::debug_make_register(format!("MYCOOLBIKE"));
        let (t2, t2_s) = Transaction::debug_make_transfer(&t0, &t0_s);
        let (t3, _) = Transaction::debug_make_transfer(&t2, &t2_s);

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);
        let block_2 = Block::new(block_1.calc_hash(), t2);
        let block_3 = Block::new(block_2.calc_hash(), t3);

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure (0)");
        chain
            .push_ignore_hash(block_1)
            .expect("Chain::push failure (1)");
        chain
            .push_ignore_hash(block_2)
            .expect("Chain::push failure (2)");
        chain
            .push_ignore_hash(block_3)
            .expect("Chain::push failure (3)");

        let q_id = "SN1337BIKE";
        let q = chain.get_blocks_for_id(q_id);
        assert_eq!(q.len(), 3, "Three blocks should match the ID");
        assert_eq!(q.iter().all(|b| b.get_data().get_id() == q_id), true);
    }

    #[test]
    fn test_query_pub_key() {
        let mut chain = Chain::new();

        // Transactions
        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let pub_key = t0.get_public_key_output().clone();
        let (t1, _) = Transaction::debug_make_register(format!("MYCOOLBIKE"));
        let (t2, t2_s) = Transaction::debug_make_transfer(&t0, &t0_s);
        let (t3, _) = Transaction::debug_make_transfer(&t2, &t2_s);

        // Blocks
        let block_0 = Block::new(chain.get_genesis_block().calc_hash(), t0);
        let block_1 = Block::new(block_0.calc_hash(), t1);
        let block_2 = Block::new(block_1.calc_hash(), t2);
        let block_3 = Block::new(block_2.calc_hash(), t3);

        chain
            .push_ignore_hash(block_0)
            .expect("Chain::push failure (0)");
        chain
            .push_ignore_hash(block_1)
            .expect("Chain::push failure (1)");
        chain
            .push_ignore_hash(block_2)
            .expect("Chain::push failure (2)");
        chain
            .push_ignore_hash(block_3)
            .expect("Chain::push failure (3)");

        let q = chain.get_blocks_for_pub_key(&pub_key);
        assert_eq!(q.len(), 2, "The two blocks between which the ownership of the bike is first transferred should be found");
    }

    #[test]
    fn test_mine() {
        let mut chain = Chain::new();

        let (t0, t0_s) = Transaction::debug_make_register(format!("SN1337BIKE"));
        let (t1, t1_s) = Transaction::debug_make_transfer(&t0, &t0_s);
        let (t2, _) = Transaction::debug_make_transfer(&t1, &t1_s);

        // First block
        let block = chain.mine(t0);
        let hash = hash::hash_to_str(&block.calc_hash());
        assert!(
            hash.ends_with(HASH_END_PATTERN),
            "Hash must end with \"{}\" (was: \"{}\")",
            HASH_END_PATTERN,
            hash
        );
        chain.push(block).expect("Chain::push failure (0)");

        // Second block
        let block = chain.mine(t1);
        let hash = hash::hash_to_str(&block.calc_hash());
        assert!(
            hash.ends_with(HASH_END_PATTERN),
            "Hash must end with \"{}\" (was: \"{}\")",
            HASH_END_PATTERN,
            hash
        );
        chain.push(block).expect("Chain::push failure (0)");

        // Third block
        let block = chain.mine(t2);
        let hash = hash::hash_to_str(&block.calc_hash());
        assert!(
            hash.ends_with(HASH_END_PATTERN),
            "Hash must end with \"{}\" (was: \"{}\")",
            HASH_END_PATTERN,
            hash
        );
        chain.push(block).expect("Chain::push failure (0)");
    }

    #[test]
    #[should_panic(expected = "Chain::push failure: BadHash")]
    fn test_bad_hash() {
        let mut chain = Chain::new();

        let (t0, _) = Transaction::debug_make_register(format!("SN1337BIKE"));

        // Make bad block
        let mut block = Block::new(chain.get_genesis_block().calc_hash(), t0);
        while hash::hash_to_str(&block.calc_hash()).ends_with(HASH_END_PATTERN) {
            block.inc_nonce();
        }

        chain.push(block).expect("Chain::push failure");
    }
}
