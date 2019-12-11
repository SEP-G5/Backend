use crate::backend::BackendErr;
use crate::blockchain::transaction::{PubKey, Transaction};
use crate::rest::server::Peers;
use futures::channel::oneshot::Sender;

// ========================================================================== //

/// Enumeration of operations that can be sent from the REST server to the
/// backend to handle specific events.
///
pub enum Operation {
    /// Operation that queries the blockchain for transactions that match a
    /// specific ID. The 'limit' specifies the number of transactions to return
    /// and 'skip' specifies the offset from the first transaction in the list.
    QueryID {
        id: String,
        limit: usize,
        skip: usize,
        res: Sender<Vec<Transaction>>,
    },
    /// Operation that queries the blockchain for transactions that are related
    /// to a specific public key. 'limit' and 'skip' behaves like for 'QueryID'
    QueryPubKey {
        key: PubKey,
        limit: usize,
        skip: usize,
        res: Sender<Vec<Transaction>>,
    },
    /// Operation that queries the backend for a list of peers that can be
    /// connected to.
    QueryPeers {
        res: Sender<Peers>,
    },
    /// Operation that signifies that a new transaction has been made and that
    /// the backend should handle it.
    CreateTransaction {
        transaction: Transaction,
        res: Sender<Result<(), BackendErr>>,
    },
    DebugDumpGraph,
}
