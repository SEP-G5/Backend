use sha2::{Digest, Sha256};
use std::convert::TryInto;

/// Alias for a type that represents a hash produced from the SHA256 digest.
///
pub type Hash = [u8; 32];

/// Value for an empty (0) hash value.
///
pub const EMPTY_HASH: Hash = [0; 32];

// ========================================================================== //

///
pub trait Hashable {
    /// Calculate the hash of the object
    fn calc_hash(&self) -> Hash;
}

// ========================================================================== //

///
pub fn obj_hash<T: AsRef<[u8]>>(object: &T) -> Hash {
    let mut hasher = Sha256::new();
    hasher.input(object);
    hasher
        .result()
        .as_slice()
        .try_into()
        .expect("Sha256 must produce a digest of 256-bits")
}

// ========================================================================== //

/// Convert a hash value to a string
///
pub fn hash_to_str(hash: &Hash) -> String {
    let parts: Vec<String> = hash.iter().map(|byte| format!("{:02x}", byte)).collect();
    parts.join("")
}

// ========================================================================== //

impl Hashable for String {
    fn calc_hash(&self) -> Hash {
        obj_hash(&self)
    }
}

impl Hashable for &str {
    fn calc_hash(&self) -> Hash {
        obj_hash(&self)
    }
}
