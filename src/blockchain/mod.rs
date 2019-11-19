mod block;
mod transaction;

// ========================================================================== //

use block::Block;
use std::fmt::{self, Display, Formatter};
//use transaction::Transaction;

// ========================================================================== //

#[derive(Debug)]
pub enum BlockchainErr {
    BadParent,
    BadTransaction,
}

// ========================================================================== //

type BlockType = Block<String>;

/// Represents the blockchain structure
///
pub struct Blockchain {
    blocks: Vec<BlockType>,
}

// ========================================================================== //

impl Blockchain {
    /// Construct a new empty blockchain.
    pub fn new() -> Blockchain {
        Blockchain { blocks: Vec::new() }
    }

    /// Push a new block at the end of the blockchain. The block is first
    /// verified using "Blockchain::check_valid()"
    ///
    pub fn push(&mut self, block: BlockType) -> Result<(), BlockchainErr> {
        Blockchain::check_valid_parent(self, &block)?;
        Blockchain::check_valid_transactions(self, &block)?;
        self.blocks.push(block);
        Ok(())
    }

    /// Checks if the specified block is a valid next block in the chain by only
    /// checking that the parent is correct
    ///
    pub fn check_valid_parent(&self, block: &BlockType) -> Result<(), BlockchainErr> {
        // Verify parent
        let prev_hash = if self.blocks.len() >= 1 {
            self.blocks[self.len() - 1].calc_hash()
        } else {
            block::EMPTY_HASH
        };
        if prev_hash != *block.get_parent_hash() {
            return Err(BlockchainErr::BadParent);
        }
        Ok(())
    }

    /// Checks if the specified block is a valid next block in the chain by
    /// checking that each transaction in the block is valid.
    ///
    pub fn check_valid_transactions(&self, block: &BlockType) -> Result<(), BlockchainErr> {
        // Is this a registration?

        // Or a transaction?

        // Find the last block that matches ID
        Ok(())
    }

    /// Returns the length of the blockchain in number of blocks stored in it.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }
}

// ========================================================================== //

impl Display for Blockchain {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let block_msg: String = self.blocks.iter().map(|b| format!("\n\t{}", b)).collect();
        write!(
            f,
            "Blockchain {{ len: {}, blocks: {}\n}}",
            self.len(),
            block_msg
        )
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
        let chain = Blockchain::new();
        assert_eq!(chain.len(), 0, "Newly created blockchains must be empty");
    }

    #[test]
    fn test_append() {
        let mut chain = Blockchain::new();

        let block_0 = Block::new(block::EMPTY_HASH, format!("First block"));
        let block_1 = Block::from_parent(&block_0, format!("Second block"));
        let block_2 = Block::from_parent(&block_1, format!("Third block"));
        let block_3 = Block::from_parent(&block_2, format!("Fourth block"));

        chain.push(block_0).expect("Failed to add block 0:");
        chain.push(block_1).expect("Failed to add block 1:");
        chain.push(block_2).expect("Failed to add block 2:");
        chain.push(block_3).expect("Failed to add block 3:");

        assert_eq!(chain.len(), 4, "Blockchain should contain 4 blocks");
    }
}
