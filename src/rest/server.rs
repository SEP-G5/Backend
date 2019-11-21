use crate::blockchain::transaction::{PubKey, Signature, Transaction};
use crate::blockchain::util::Timestamp;
use base64::{decode_config, encode};
use rocket::{self, *};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{self, Value};

// ============================================================ //

/// main entry point for the REST server program
pub fn run_server() {
    rocket::ignite()
        .mount("/", routes![index, new, info, peer])
        .launch();
}

// ============================================================ //

#[derive(Serialize, Deserialize)]
struct Response {
    ok: bool,
    msg: String,
}

#[derive(Serialize, Deserialize)]
struct Peer {
    ip: String,
}

#[derive(Serialize, Deserialize)]
struct Peers {
    peers: Vec<Peer>,
}

// ============================================================ //

fn make_response(ok: bool, msg: &str) -> String {
    let r = Response {
        ok,
        msg: String::from(msg),
    };
    serde_json::to_string(&r).expect("failed to convert to json")
}

fn json_transaction_to_transaction(v: Value) -> Result<Transaction, String> {
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
#[post("/new", format = "json", data = "<data>")]
fn new(data: String) -> String {
    let v: Value = match serde_json::from_str(data.as_str()) {
        Ok(v) => v,
        Err(e) => return make_response(false, &format!("{}", e)),
    };

    let t: Transaction = match json_transaction_to_transaction(v) {
        Ok(t) => t,
        Err(s) => return s,
    };

    // TODO ask blockchain to accept the trasaction
    // ...

    // TEMP CODE verify the transaction, will be done by blockchain later
    match t.verify() {
        Ok(_) => {}
        Err(e) => return make_response(false, &format!("{}", e)),
    }

    make_response(
        false,
        "The transaction was accepted! (maybe, not implemented yet ;)",
    )
}

/// The client wants information about the given data. Such as information
/// about an id, or public key.
#[post("/info", format = "json", data = "<data>")]
fn info(data: String) -> String {
    let v: Value = match serde_json::from_str(data.as_str()) {
        Ok(j) => j,
        Err(e) => return make_response(false, &format!("{}", e)),
    };

    // cannot be both and cannot be none
    let pk: bool = v["publicKey"] != Value::Null;
    let id: bool = v["id"] != Value::Null;

    if pk && !id {
        // TODO ask the blockchain for all transactions with a given pk
        // ...
    } else if id && !pk {
        // TODO ask the blockchain for all transactions with a given id
        // ...
    } else if pk && id {
        return make_response(
            false,
            "info request has both public key and id, can only have one.",
        );
    } else if !pk && !id {
        return make_response(false, "info request has no public key or id, must have one");
    }

    // TODO not have this default fail msg
    return make_response(
        false,
        "no transaction with that public key was found in the blockchain",
    );
}

#[get("/peer")]
fn peer() -> String {
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
}
