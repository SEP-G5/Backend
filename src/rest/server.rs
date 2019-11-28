use crate::backend::{operation::Operation, BackendErr};
use crate::blockchain::transaction::{PubKey, Signature, Transaction};
use crate::blockchain::util::Timestamp;
use base64::{decode_config, encode};
use futures::channel::oneshot;
use rocket::{self, http::Status, *};
use serde::{Deserialize, Serialize};
use serde_json::{self, json, Value};
use std::sync::mpsc;
use std::sync::Mutex;

// ============================================================ //

/// main entry point for the REST server program
pub fn run_server(sender: mpsc::Sender<Operation>) {
    rocket::ignite()
        .mount("/", routes![index, tx_post, tx_get, peer])
        .manage(Mutex::new(sender))
        .launch();
}

// ============================================================ //
// structs
// ============================================================ //

#[derive(Serialize, Deserialize)]
struct Response {
    ok: bool,
    msg: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Peer {
    pub ip: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Peers {
    pub peers: Vec<Peer>,
}

#[derive(Serialize, Deserialize)]
struct Transactions {
    handle: Vec<Transaction>,
}

#[derive(Serialize, Deserialize)]
struct JsonTransactions {
    handle: Vec<Value>,
}

// ============================================================ //
// impls
// ============================================================ //

impl Transactions {
    pub fn iter(&self) -> std::slice::Iter<Transaction> {
        self.handle.iter()
    }
}

impl JsonTransactions {
    #[allow(dead_code)]
    fn new() -> JsonTransactions {
        JsonTransactions { handle: Vec::new() }
    }

    fn from(t: Transactions) -> JsonTransactions {
        let vv: Vec<Value> = t
            .iter()
            .map(|tx| transaction_to_json_transaction(tx))
            .collect();
        JsonTransactions { handle: vv }
    }

    fn to_string(&self) -> String {
        serde_json::to_string(&self.handle).expect("failed to convert to json")
    }
}

// ============================================================ //
// hepler functions
// ============================================================ //

fn make_response(ok: bool, msg: &str) -> String {
    let r = Response {
        ok,
        msg: String::from(msg),
    };
    serde_json::to_string(&r).expect("failed to convert to json")
}

/// @pre t Must be a valid transaction
fn transaction_to_json_transaction(t: &Transaction) -> Value {
    let mut v: Value = json!({
        "id": t.get_id(),
        "timestamp": t.get_timestamp(),
        "publicKeyInput": Value::Null,
        "publicKeyOutput": encode(t.get_public_key_output()),
        "signature": encode(t.get_signature()),
    });
    if let Some(pk) = t.get_public_key_input() {
        *v.get_mut("publicKeyInput").unwrap() = json!(encode(pk));
    }
    v
}

fn json_transaction_to_transaction(v: &Value) -> Result<Transaction, String> {
    let id: String = match v["id"].as_str() {
        Some(s) => s.to_string(),
        None => return Err(make_response(false, "Could not parse id as String")),
    };

    let timestamp: Timestamp = match v["timestamp"].as_u64() {
        Some(v) => v,
        None => return Err(make_response(false, "Could not parse id as u64")),
    };

    let pub_key_input: Option<PubKey> = match v["publicKeyInput"].as_str() {
        Some(s) => match decode_config(s, base64::STANDARD) {
            Ok(v) => Some(v),
            Err(e) => {
                return Err(make_response(
                    false,
                    &format!(
                        "Could not decode publicKeyInput from base64 with error: {}",
                        e
                    ),
                ))
            }
        },
        None => None,
    };

    let pub_key_output: PubKey = match v["publicKeyOutput"].as_str() {
        Some(s) => match decode_config(s, base64::STANDARD) {
            Ok(v) => v,
            Err(e) => {
                return Err(make_response(
                    false,
                    &format!(
                        "Could not decode publicKeyOutput from base64 with error: {}",
                        e
                    ),
                ))
            }
        },
        None => {
            return Err(make_response(
                false,
                "Could not parse publicKeyOutput as String",
            ))
        }
    };

    let signature: Signature = match v["signature"].as_str() {
        Some(s) => match decode_config(s, base64::STANDARD) {
            Ok(v) => v,
            Err(e) => {
                return Err(make_response(
                    false,
                    &format!("Could not decode signature from base64 with error: {}", e),
                ))
            }
        },
        None => return Err(make_response(false, "Could not parse signature as String")),
    };

    Ok(Transaction::from_details(
        id,
        timestamp,
        pub_key_input,
        pub_key_output,
        signature,
    ))
}

// ============================================================ //
// paths
// ============================================================ //

#[get("/")]
fn index() -> &'static str {
    "https://github.com/SEP-G5/Backend/issues/4"
}

/// The client wants to create a new transaction.
/// @param data Contains the transaction that was created by the client
#[post("/transaction", format = "json", data = "<data>")]
fn tx_post(data: String, sender: State<Mutex<mpsc::Sender<Operation>>>) -> String {
    let v: Value = match serde_json::from_str(data.as_str()) {
        Ok(v) => v,
        Err(e) => return make_response(false, &format!("{}", e)),
    };

    let t: Transaction = match json_transaction_to_transaction(&v) {
        Ok(t) => t,
        Err(s) => return s,
    };

    // TEMP CODE verify the transaction, will be done by blockchain later
    match t.verify() {
        Ok(_) => {}
        Err(e) => return make_response(false, &format!("{}", e)),
    }

    let (res_write, mut res_read) = oneshot::channel();
    let tmp_op = Operation::CreateTransaction {
        transaction: t,
        res: res_write,
    };
    sender
        .lock()
        .expect("Failed to lock state mutex")
        .send(tmp_op)
        .expect("Failed to send op");
    let result = 'wait_loop: loop {
        match res_read.try_recv() {
            Ok(o_ec) => {
                if let Some(ec) = o_ec {
                    break 'wait_loop ec;
                }
            }
            Err(_) => break 'wait_loop Err(BackendErr::OpCancelled),
        };
        std::thread::sleep(std::time::Duration::from_millis(5));
    };

    match result {
        Ok(_) => make_response(true, &format!("The transaction was accepted")),
        Err(e) => make_response(true, &format!("The transaction was rejected: {:?}", e)),
    }
}

/// The client wants information about the given data. Such as information
/// about an id, or public key.
#[get("/transaction?<id>&<publicKey>&<limit>&<skip>", format = "json")]
#[allow(non_snake_case)]
fn tx_get(
    id: Option<String>,
    publicKey: Option<String>,
    limit: Option<u64>,
    skip: Option<u64>,
) -> Result<String, Status> {
    let pk = publicKey;

    let dummy_response = |id: &String| -> String {
        let (t0, sk0) = Transaction::debug_make_register(id.clone());
        let (t1, sk1) = Transaction::debug_make_transfer(&t0, &sk0);
        let (t2, _) = Transaction::debug_make_transfer(&t1, &sk1);

        let mut ts = Transactions { handle: Vec::new() };
        ts.handle.push(t0);
        ts.handle.push(t1);
        ts.handle.push(t2);

        let jt = JsonTransactions::from(ts);
        jt.to_string()
    };

    // TODO limit & skip should not be noop's

    if id.is_none() && pk.is_some() {
        // TODO ask the blockchain for all transactions with a given pk
        // ...
        return Ok(dummy_response(&pk.unwrap()));
    } else if id.is_some() && pk.is_none() {
        // TODO ask the blockchain for all transactions with a given id
        // ...
        return Ok(dummy_response(&id.unwrap()));
    } else if pk.is_some() && id.is_some() {
        return Err(Status {
            code: 400,
            reason: "info request has both public key and id, can only have one.",
        });
    } else {
        /*if pk.is_none() && id.is_none() */
        return Err(Status {
            code: 400,
            reason: "info request has no public key or id, must have one",
        });
    }

    // TODO return status code 401 when:
    //  * In a Registration, the bike has already an owner
    //  * In a Transfer, the public key is not the owner of the bike
}

#[get("/peer")]
fn peer(sender: State<Mutex<mpsc::Sender<Operation>>>) -> String {
    let (res_write, mut res_read) = oneshot::channel();
    let op = Operation::QueryPeers { res: res_write };
    sender
        .lock()
        .expect("Failed to lock state mutex")
        .send(op)
        .expect("Failed to send op");
    let peers = 'wait_loop: loop {
        match res_read.try_recv() {
            Ok(o_peers) => {
                if let Some(peers) = o_peers {
                    break 'wait_loop peers;
                }
            }
            Err(_) => break 'wait_loop Peers { peers: Vec::new() },
        };
        std::thread::sleep(std::time::Duration::from_millis(5));
    };

    serde_json::to_string(&peers).expect("failed to convert to json")

    /*
        // TODO ask the network for list of peers
        let mut p = Peers { peers: Vec::new() };
        p.peers.push(Peer {
            ip: format!("123.123.123.123"),
        });
        p.peers.push(Peer {
            ip: format!("1.2.3.4"),
        });
        p.peers.push(Peer {
            ip: format!("11.22.33.44"),
        });
        p.peers.push(Peer {
            ip: format!("101.202.30.40"),
        });
        serde_json::to_string(&p).expect("failed to convert to json")
    */
}
