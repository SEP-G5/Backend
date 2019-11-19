mod block;
mod hash;
mod transaction;
mod util;

// ========================================================================== //

use block::Block;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use transaction::Transaction;

// ========================================================================== //

#[derive(Debug)]
pub enum BlockchainErr {
    BadParent,
    NonUniqueTransactionID,
    BadTransaction(String),
}

// ========================================================================== //

type BlockType = Block<Transaction>;

/// Represents the blockchain structure
///
#[derive(Serialize, Deserialize)]
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
            hash::EMPTY_HASH
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
        // Verify transaction
        if let Err(e) = block.get_data().verify() {
            return Err(BlockchainErr::BadTransaction(e));
        }
        // Is this a registration?
        if !block.get_data().has_input() {
            // Check for unique ID
            for b in self.blocks.iter().rev() {
                if b.get_data().get_id() == block.get_data().get_id() {
                    return Err(BlockchainErr::NonUniqueTransactionID);
                }
            }
        } else {
            // Or a transaction?
            let mut prev: Option<&BlockType> = None;
            for b in self.blocks.iter().rev() {
                if b.get_data().get_id() == block.get_data().get_id() {
                    prev = Some(&b);
                    break;
                }
            }

            match prev {
                Some(b) => {
                    if !block.get_data().verify_is_next(b.get_data()) {
                        return Err(BlockchainErr::BadTransaction(format!(
                            "Input to transaction is not valid"
                        )));
                    }
                }
                None => {
                    return Err(BlockchainErr::BadTransaction(format!(
                        "Input to transaction does not exist"
                    )))
                }
            }
        }

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
    use rust_sodium::crypto::sign::{self, ed25519::PublicKey, ed25519::SecretKey};

    #[test]
    fn test_create() {
        let chain = Blockchain::new();
        assert_eq!(chain.len(), 0, "Newly created blockchains must be empty");
    }

    #[test]
    fn test_append() {
        let mut chain = Blockchain::new();

        // First transaction
        let (t0_p, t0_s) = sign::gen_keypair();
        let mut t0 = Transaction::new(format!("SN1337BIKE"), None, t0_p.as_ref().to_vec());
        t0.sign(&t0_s);

        // Second transaction
        let (t1_p, _) = sign::gen_keypair();
        let mut t1 = Transaction::new_debug(
            t0.get_id().clone(),
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

        assert_eq!(chain.len(), 2, "Blockchain should contain 4 blocks");
    }

    #[test]
    #[should_panic]
    fn test_append_invalid_id() {
        let mut chain = Blockchain::new();

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
}
