use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

// ========================================================================== //

/// Alias for a type that represents a hash produced from the SHA256 digest.
///
type HashType = [u8; 32];

// ========================================================================== //

/// Represents a block in the blockchain
///
#[derive(Serialize, Deserialize)]
pub struct Block<T>
where
    T: AsRef<[u8]> + Serialize + Deserialize,
{
    /// Hash of the parent block
    parent: u64,
    /// Timestamp for when the block was generated. Specified in UNIX epoch
    timestamp: u128,
    /// Random nonce
    rand_nonce: u64,
    /// Nonce. Changed to produce correct hash
    nonce: u64,
    /// Data
    data: T,
}

// ========================================================================== //

impl<T: AsRef<[u8]>, Serialize, Deserialize> Block<T> {
    /// Create a new block with the specified parent and data
    ///
    pub fn new(parent: u64, data: T) -> Block<T> {
        let rand_nonce = 0;
        let nonce = 0;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Could not retrieve a correct UNIX epoch timestamp")
            .as_micros();
        Block {
            parent,
            timestamp,
            rand_nonce,
            nonce,
            data,
        }
    }

    /// Returns the timestamp when the block was created. In UNIX epoch
    ///
    pub fn get_timestamp(&self) -> u128 {
        self.timestamp
    }

    /// Returns the current nonce value of the block.
    ///
    pub fn get_nonce(&self) -> u64 {
        self.nonce
    }

    /// Set the nonce of the block to the specified value.
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
    pub fn calc_hash(&self) -> HashType {
        let mut hasher = Sha256::new();
        hasher.input(self.parent.to_le_bytes());
        hasher.input(self.timestamp.to_le_bytes());
        hasher.input(self.nonce.to_le_bytes());
        hasher.input(&self.data);
        hasher
            .result()
            .as_slice()
            .try_into()
            .expect("Sha256 must produce a digest of 256-bits")
    }
}

// ========================================================================== //

/// Convert a hash value to a string
///
pub fn hash_to_str(hash: &HashType) -> String {
    let parts: Vec<String> = hash.iter().map(|byte| format!("{:02x}", byte)).collect();
    parts.join("")
}

// ========================================================================== //
// Tests
// ========================================================================== //

#[cfg(test)]
mod tests {
    use super::*;

    /// Test for the creation of a new block
    ///
    #[test]
    fn test_block_create() {
        let block = Block::<&str>::new(0, "Hello");
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
        let block = Block::<&str>::new(0, "World");

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
        let block_0 = Block::<&str>::new(0, "a string");
        let block_1 = Block::<&str>::new(0, "a string");
        assert!(block_0.get_timestamp() <= block_1.get_timestamp());
    }

    /// Test serialization
    ///
    #[test]
    fn test_block_serde() {
        let block = Block::<&str>::new(0, "some str");

        //let ser = serde_
    }
}
