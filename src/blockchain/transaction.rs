use crate::blockchain::hash::{self, Hash, Hashable};
use crate::blockchain::util;
use rust_sodium::crypto::sign::{
    self, ed25519::sign, ed25519::verify, ed25519::PublicKey, ed25519::SecretKey,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Display, Formatter};

type PubKey = Vec<u8>;
type Signature = Vec<u8>;

/// Future work: PubKey and Signature should be fixed size arrays.
#[derive(Serialize, Deserialize, Clone)]
pub struct Transaction {
    /// bike SN
    id: String,
    /// seconds since unix epoch (1970)
    timestamp: util::Timestamp,
    pub_key_input: Option<PubKey>,
    pub_key_output: PubKey,
    signature: Signature,
}

/// Helper function to format a Vec<u8> buffer to string as hex rep.
pub fn buf_to_str(buf: &Vec<u8>) -> String {
    let parts: Vec<String> = buf.iter().map(|byte| format!("{:02x}", byte)).collect();
    parts.join("")
}

/// Allow transactions to be printed.
impl Display for Transaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let k_input = if self.pub_key_input.is_some() {
            buf_to_str(&self.pub_key_input.as_ref().unwrap())
        } else {
            format!("None")
        };
        write!(
            f,
            "Transaction:{{ id: {}, timestamp: {}, public_key_input: {:#?}, \
             public_key_output: {:#?}, signature: {:#?} }}",
            self.id,
            self.timestamp,
            k_input,
            buf_to_str(&self.pub_key_output),
            buf_to_str(&self.signature)
        )
    }
}

impl Transaction {
    pub fn new(id: String, pub_key_input: Option<PubKey>, pub_key_output: PubKey) -> Transaction {
        Transaction {
            id: id,
            timestamp: util::make_timestamp(),
            pub_key_input: pub_key_input,
            pub_key_output: pub_key_output,
            signature: Vec::new(),
        }
    }

    /// Make debug transaction
    /*pub fn new_debug(
        id: String,
        timestamp: util::Timestamp,
        pub_key_input: Option<PubKey>,
        pub_key_output: PubKey,
        signature: Signature,
    ) -> Transaction {
        Transaction {
            id,
            timestamp,
            pub_key_input,
            pub_key_output,
            signature,
        }*/

    /// @param id The id of the item, such as serial number of a bike.
    pub fn debug_make_register(id: String) -> (Transaction, SecretKey) {
        let (pk, sk) = sign::gen_keypair();
        let mut t = Transaction::new(id, None, pk.as_ref().to_vec());
        t.sign(&sk);
        (t, sk)
    }

    /// @param t_prev The previous transaction
    /// @param t_sk The previous secret key
    pub fn debug_make_transfer(
        t_prev: &Transaction,
        sk_prev: &SecretKey,
    ) -> (Transaction, SecretKey) {
        let (pk, sk) = sign::gen_keypair();
        let mut t = Transaction {
            id: t_prev.id.clone(),
            timestamp: util::make_timestamp(),
            pub_key_input: Some(t_prev.pub_key_output.clone()),
            pub_key_output: pk.as_ref().to_vec(),
            signature: Vec::new(),
        };
        t.sign(&sk_prev);
        (t, sk)
    }

    /// Sign a transaction. Make sure all data is filled in, except
    /// signature. Store the signature in itself.
    pub(crate) fn sign(&mut self, sk: &SecretKey) {
        let buf = self.content_to_u8();
        let sig = sign(buf.as_slice(), &sk);
        self.signature = sig;
    }

    /// Verify that this transaction is a valid next transaction, given that the
    /// previous transaction was "prev_t".
    /// @pre "prev_t" must be a valid transaction.
    pub fn verify_is_next(&self, prev_t: &Transaction) -> bool {
        match self.verify() {
            Ok(_) => match &self.pub_key_input {
                Some(key) => &prev_t.pub_key_output == key,
                None => false,
            },
            Err(_) => false,
        }
    }

    /// Verify if the transaction is valid (does the signature match the content?).
    /// There are two types of transactions that are verified differently.
    ///   "Register": There is no input, use the public key of the output.
    ///   "Transfer": There is a input, use the public key of the input.
    pub fn verify(&self) -> Result<(), String> {
        let do_verify = |pk: &[u8], sig: &[u8]| -> Result<(), String> {
            let pk = PublicKey::from_slice(pk);
            let pk = match pk {
                Some(p) => p,
                None => return Err(format!("could not parse public key")),
            };
            match verify(sig, &pk) {
                Ok(m) => {
                    let content = self.content_to_u8();
                    if content == m {
                        return Ok(());
                    } else {
                        return Err(format!("content does not match the signature"));
                    }
                }
                Err(_) => return Err(format!("signature is not valid")),
            };
        };

        match &self.pub_key_input {
            Some(pub_key_input) => {
                return do_verify(pub_key_input.as_slice(), self.signature.as_slice());
            }
            None => {
                return do_verify(self.pub_key_output.as_slice(), self.signature.as_slice());
            }
        }
    }

    /// Copy the content of the transaction into a buffer
    fn content_to_u8(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::from(self.id.as_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        if let Some(ref key) = self.pub_key_input {
            buf.extend(key);
        }
        buf.extend(&self.pub_key_output);
        buf
    }

    /// Returns whether or not the transaction
    pub fn has_input(&self) -> bool {
        self.pub_key_input.is_some()
    }

    /// Returns the ID of the transacted object
    pub fn get_id(&self) -> &String {
        &self.id
    }

    /// Returns the output public key
    ///
    pub fn get_public_key_output(&self) -> &PubKey {
        &self.pub_key_output
    }
}

impl Hashable for Transaction {
    fn calc_hash(&self) -> Hash {
        hash::obj_hash(&self.signature)
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        // make a transaction, sign it and verify
        let (mut t, sk) = Transaction::debug_make_register(format!("SN1337BIKE"));
        t.sign(&sk);
        assert_eq!(t.verify(), Ok(()));

        // tamper with the transaction content
        t.id += "1";
        assert_ne!(t.verify(), Ok(()));
    }

    #[test]
    fn test_verify_is_next() {
        // T0 - make the first "register" transaction
        let (t0, sk0) = Transaction::debug_make_register(format!("SN1337BIKE"));
        assert_eq!(t0.verify(), Ok(()));

        // T1 - make the second "transfer" transaction
        let (t1, sk1) = Transaction::debug_make_transfer(&t0, &sk0);
        assert_eq!(t1.verify(), Ok(()));
        assert_eq!(t1.verify_is_next(&t0), true);

        // T2 - make the third "transfer" transaction
        let (t2, sk2) = Transaction::debug_make_transfer(&t1, &sk1);
        assert_eq!(t2.verify(), Ok(()));
        assert_eq!(t2.verify_is_next(&t1), true);
    }
}
