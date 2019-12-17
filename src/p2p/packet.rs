use crate::blockchain::{block::Block, hash::Hash, transaction::Transaction};
use bytes::buf::BufMut;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::net::SocketAddr;
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
    /// Packet that is used to ask for a block with the specified hash
    GetBlockByHash(Hash),

    PeerShuffleReq(Vec<SocketAddr>),

    PeerShuffleResp(Option<Vec<SocketAddr>>),

    /// Packet that is sent when node has been notified of a new transaction
    /// from the frontend.
    PostTx(Transaction),

    /// A node wants to join the p2p network.
    JoinReq(u16),

    /// A node has joined the p2p network, and you should add them aswel.
    JoinFwd(SocketAddr),

    /// Used to send close connection request from backend to node.
    CloseConnection(),
}

impl Packet {
    fn from_bytes_mut(buf: &mut BytesMut) -> Result<Option<Packet>, PacketErr> {
        //let packet_buf = buf.split();
        let packet = match bincode::deserialize(&buf) {
            Ok(packet) => packet,
            Err(e) => match *e {
                bincode::ErrorKind::Io(e) => match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => return Ok(None),
                    _ => {
                        return Err(PacketErr::Deserialize(format!(
                            "failed to deserialize packet (size: {}) [{:?}]",
                            buf.len(),
                            e
                        )))
                    }
                },
                _ => {
                    return Err(PacketErr::Deserialize(format!(
                        "failed to deserialize packet (size: {}) [{:?}]",
                        buf.len(),
                        e
                    )))
                }
            },
        };
        let size = bincode::serialized_size(&packet).unwrap() as usize;
        buf.split_to(size);
        Ok(Some(packet))
    }

    fn to_bytes_mut(&self, buf: &mut BytesMut) -> Option<PacketErr> {
        // TODO we are allocating one extra packet here, we shouldn't
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
            Packet::from_bytes_mut(buf)
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
