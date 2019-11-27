use std::collections::HashMap;
use std::net::SocketAddr;
use crate::p2p::network::Tx;

pub struct Shared {
    /// Maps nodes to the send half of the backend-to-network channel
    pub b2n_tx: HashMap<SocketAddr, Tx>,

    /// network-to-backend channel, for use by the nodes when sending to backend.
    pub n2b_tx: Tx,
}

impl Shared {
    pub fn new(tx: Tx) -> Shared {
        Shared {
            b2n_tx: HashMap::new(),
            n2b_tx: tx,
        }
    }
}
