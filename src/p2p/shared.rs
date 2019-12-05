use crate::p2p::network::Tx;
use std::collections::HashMap;
use std::net::SocketAddr;
//use tokiotask::Task;

/*
/// Backend-to-Network communication structure.
pub struct B2NTx {
    /// transmit channel
    tx: Tx,
    /// Task where the transmitted message will be received.
    task: Task,
}

impl B2NTx {
    pub fn new(tx: Tx, task: Task) -> B2NTx {
        B2NTx {tx, task}
    }

    pub fn send(&self, packet: Packet) -> Result<(), ()> {
        let res = self.tx.send(packet);
        self.task.notify();
        match res {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
}
*/

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
