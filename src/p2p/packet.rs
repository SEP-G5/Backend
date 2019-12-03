use crate::blockchain::{block::Block, transaction::Transaction};
use bytes::buf::BufMut;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::io;
use tokio_util::codec::{Decoder, Encoder};

// ============================================================ //

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Packet {
    /// Packet that is used to post a block after it has been mined. Anyone
    /// receiving this packet should verify the block and then add it to it's
    /// own blockchain.
    PostBlock(Option<Block<Transaction>>, u64),
    /// Packet that is used to ask for a block at the specified index in the
    /// blockchain.
    GetBlock(u64),
    /// Packet that is used to post a list of known peers to others in the
    /// network. When this packet is received the peers should be added to those
    /// that are known.
    PostPeers,
    /// Packet that is sent to ask for a list of known peers. Anyone receiving
    /// this should respond with their known list of peers.
    GetPeers,
    /// Packet that is sent when node has been notified of a new transaction
    /// from the frontend.
    PostTx(Transaction),
}

impl Packet {
    fn from_bytes_mut(buf: &mut BytesMut) -> Result<Packet, PacketErr> {
        let packet_buf = buf.split();
        match bincode::deserialize(&packet_buf) {
            Ok(packet) => Ok(packet),
            Err(e) => Err(PacketErr::Deserialize(format!(
                "failed to deserialize packet [{:?}]",
                e
            ))),
        }
    }

    fn to_bytes_mut(&self, buf: &mut BytesMut) -> Option<PacketErr> {
        /*
        let len = match bincode::serialized_size(self) {
            Ok(len) => len as usize,
            Err(e) => return Some(PacketErr::Other(format!("could not estimate packet size [{:?}]", e))),
        };
        buf.reserve(len);

        match bincode::serialize_into(buf.as_mut(), self) {
        Ok(_) => None,
        Err(e) => Some(PacketErr::Serialize(format!(
            "failed to serialize packet [{:?}]",e))),
        }
         */

        // TODO we are allocating one extra packet here, we shouldnt
        let tmp_vec = match bincode::serialize(self) {
            Ok(tmp_vec) => tmp_vec,
            Err(e) => {
                return Some(PacketErr::Serialize(format!(
                    "failed to serialize packet [{:?}]",
                    e
                )))
            }
        };

        buf.reserve(tmp_vec.len());
        buf.put_slice(tmp_vec.as_slice());
        None
    }
}

#[derive(Debug)]
pub enum PacketErr {
    Deserialize(String),
    Serialize(String),
    Other(String),
}

impl fmt::Display for PacketErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketErr::Deserialize(s) => write!(f, "{}", s),
            PacketErr::Serialize(s) => write!(f, "{}", s),
            PacketErr::Other(s) => write!(f, "{}", s),
        }
    }
}

impl From<io::Error> for PacketErr {
    fn from(e: io::Error) -> Self {
        PacketErr::Other(format!("{}", e))
    }
}

// ============================================================ //

#[derive(Clone, Debug)]
pub struct PacketCodec(());

impl PacketCodec {
    pub fn new() -> PacketCodec {
        PacketCodec(())
    }
}

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = PacketErr;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if !buf.is_empty() {
            match Packet::from_bytes_mut(buf) {
                Ok(packet) => Ok(Some(packet)),
                Err(e) => Err(e),
            }
        } else {
            Ok(None)
        }
    }
}

impl Encoder for PacketCodec {
    type Item = Packet;
    type Error = PacketErr;

    fn encode(&mut self, packet: Packet, buf: &mut BytesMut) -> Result<(), PacketErr> {
        match packet.to_bytes_mut(buf) {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }
}
