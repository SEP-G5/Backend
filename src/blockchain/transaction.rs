use rust_sodium::crypto::sign::{
    self, ed25519::sign, ed25519::verify, ed25519::PublicKey, ed25519::SecretKey,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{self, Display, Formatter};
use std::time::SystemTime;

type Timestamp = u64;
type PubKey = Vec<u8>;
type Signature = Vec<u8>;

/// Future work: PubKey and Signature should be fixed size arrays.
#[derive(Serialize, Deserialize, Clone)]
pub struct Transaction {
    /// bike SN
    id: String,
    /// seconds since unix epoch (1970)
    timestamp: Timestamp,
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
             public_key_output: {:#?}, signature: {:#?}}}",
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
            timestamp: make_timestamp(),
            pub_key_input: pub_key_input,
            pub_key_output: pub_key_output,
            signature: Vec::new(),
        }
    }

    /// Sign a transaction. Make sure all data is filled in, except
    /// signature. Store the signature in itself.
    fn sign(&mut self, sk: &SecretKey) {
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
}

fn make_timestamp() -> Timestamp {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("failed to make timestamp");
    return ts.as_secs() as Timestamp;
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        // make a transaction, sign it and verify
        let (pk, sk) = sign::gen_keypair();
        let mut t = Transaction::new(format!("SN1337BIKE"), None, pk.as_ref().to_vec());
        t.sign(&sk);
        assert_eq!(t.verify(), Ok(()));

        // tamper with the transaction content
        t.id += "1";
        assert_ne!(t.verify(), Ok(()));
    }

    #[test]
    fn test_verify_is_next() {
        // T0 - make the first "register" transaction
        let (pk_0, sk_0) = sign::gen_keypair();
        let mut t_0 = Transaction::new(format!("SN1337BIKE"), None, pk_0.as_ref().to_vec());
        t_0.sign(&sk_0);
        assert_eq!(t_0.verify(), Ok(()));

        // T1 - make the second "transfer" transaction
        let (pk_1, sk_1) = sign::gen_keypair();
        let mut t_1 = Transaction {
            id: t_0.id.clone(),
            timestamp: make_timestamp(),
            pub_key_input: Some(t_0.pub_key_output.clone()),
            pub_key_output: pk_1.as_ref().to_vec(),
            signature: Vec::new(),
        };
        t_1.sign(&sk_0); // We use the private key of t_0 here!
        assert_eq!(t_1.verify(), Ok(()));
        assert_eq!(t_1.verify_is_next(&t_0), true);

        // T2 - make the third "transfer" transaction
        let (pk_2, sk_2) = sign::gen_keypair();
        let mut t_2 = Transaction {
            id: t_1.id.clone(),
            timestamp: make_timestamp(),
            pub_key_input: Some(t_1.pub_key_output.clone()),
            pub_key_output: pk_2.as_ref().to_vec(),
            signature: Vec::new(),
        };
        t_2.sign(&sk_1); // We use the private key of t_1 here!
        assert_eq!(t_2.verify(), Ok(()));
        assert_eq!(t_2.verify_is_next(&t_1), true);
    }
}
