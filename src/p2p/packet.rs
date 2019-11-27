use crate::blockchain::{block::Block, transaction::Transaction};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Packet {
    PostBlock(Block<Transaction>),
    GetBlock(u64),
    PostPeers(),
    GetPeers,
    PostTx(Transaction),
}

/*
#[repr(u32)]
enum PacketType {
    PostBlock = 0,
    GetPeers = 1,
    GetBlock = 2,
    PostTx = 3,
}

struct Packet {
    buf: Vec<u8>,
}

const HEADER_LEN: usize = 12;

fn set_header(packet_type: PacketType, payload_len: u64,
              buf: &mut [u8; HEADER_LEN]) {

}

impl Packet {



    pub fn new(capacity: usize) -> Packet {
        Packet {
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Can fail if header is invalid
    pub fn from(buf: &[u8]) -> Result<Packet, ()> {
        if buf.len() < HEADER_LEN + 1 {
            return Err(());
        }

        let packet = Packet::new(buf.len());


        Ok(packet)
    }

    pub fn set_payload(&self, payload: &[u8]) {
        if self.buf.len() > HEADER_LEN {
            self.buf.truncate(HEADER_LEN);
        }
        self.buf.reserve(payload.len() + HEADER_LEN); // this needed?
        self.buf.extend_from_slice(payload);
    }

    pub fn get_payload(&self) -> &[u8] {
        &self.buf[HEADER_LEN..]
    }

    pub fn to(&self) -> (Header, Bytes) {

    }
}
*/
