use crate::blockchain::hash::{self, Hash, Hashable};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

// ========================================================================== //

// ========================================================================== //

/// Represents a block in the blockchain
///
#[derive(Serialize, Deserialize, Debug)]
pub struct Block<T: Hashable> {
    /// Hash of the parent block
    parent: Hash,
    /// Timestamp for when the block was generated in seconds since UNIC epoch.
    timestamp: u64,
    /// Random nonce
    rand_nonce: u64,
    /// Nonce. Changed to produce correct hash
    nonce: u64,
    /// Data
    data: T,
}

// ========================================================================== //

impl<T: Hashable> Block<T> {
    /// Create a new block with the specified parent and data
    ///
    pub fn new(parent: Hash, data: T) -> Block<T> {
        let mut rng = rand::thread_rng();
        let rand_nonce: u64 = rng.gen();
        let nonce = 0;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Could not retrieve a correct UNIX epoch timestamp")
            .as_secs() as u64;
        Block {
            parent,
            timestamp,
            rand_nonce,
            nonce,
            data,
        }
    }

    /// Create a block from a parent block. The hash for the parent block is
    /// calculated before the children is created.
    ///
    pub fn from_parent(parent: &Block<T>, data: T) -> Block<T> {
        let parent_hash = parent.calc_hash();
        Block::new(parent_hash, data)
    }

    /// Returns the timestamp when the block was created. In UNIX epoch
    ///
    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Returns the current nonce value of the block.
    ///
    pub fn get_nonce(&self) -> u64 {
        self.nonce
    }

    /// Set the noHashTypek to the specified value.
    ///
    pub fn set_nonce(&mut self, nonce: u64) {
        self.nonce = nonce;
    }

    pub fn inc_nonce(&mut self) {
        self.set_nonce(self.get_nonce() + 1);
    }

    /// Returns a reference to the data that is currently stored in the block.
    ///
    pub fn get_data(&self) -> &T {
        &self.data
    }

    /// Calculate the hash of the block.
    ///
    pub fn calc_hash(&self) -> Hash {
        let mut hasher = Sha256::new();
        hasher.input(self.parent);
        hasher.input(self.timestamp.to_le_bytes());
        hasher.input(self.rand_nonce.to_le_bytes());
        hasher.input(self.nonce.to_le_bytes());
        hasher.input(self.data.calc_hash());
        hasher
            .result()
            .as_slice()
            .try_into()
            .expect("Sha256 must produce a digest of 256-bits")
    }
}

// ========================================================================== //

impl<T: Hashable + PartialEq> PartialEq for Block<T> {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent
            && self.timestamp == other.timestamp
            && self.rand_nonce == other.rand_nonce
            && self.nonce == other.nonce
            && self.data == other.data
    }
}

// ========================================================================== //
// Tests
// ========================================================================== //

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    /// Test for the creation of a new block
    ///
    #[test]
    fn test_block_create() {
        let block = Block::<&str>::new(hash::EMPTY_HASH, "Hello");
        assert_eq!(
            block.get_nonce(),
            0,
            "Nonce of newly created blocks must be 0"
        );
        assert_eq!(*block.get_data(), "Hello");
    }

    /// Test the calculation of the hash for a block
    ///
    #[test]
    fn test_block_hash() {
        let block = Block::<&str>::new(hash::EMPTY_HASH, "World");

        assert_eq!(
            block.calc_hash(),
            block.calc_hash(),
            "Calculating the hash multiple times must produce the same hash value"
        );
    }

    /// Test timestamp
    ///
    #[test]
    fn test_block_timestamp() {
        let block_0 = Block::<&str>::new(hash::EMPTY_HASH, "a string");
        let block_1 = Block::<&str>::new(hash::EMPTY_HASH, "a string");
        assert!(block_0.get_timestamp() <= block_1.get_timestamp());
    }

    /// Test serialization
    ///
    #[test]
    fn test_block_serde() {
        let block = Block::<&str>::new(hash::EMPTY_HASH, "some str");
        let ser = serde_json::to_string(&block).unwrap();
        let de: Block<&str> = serde_json::from_str(&ser).unwrap();
        assert_eq!(block, de);
    }
}
