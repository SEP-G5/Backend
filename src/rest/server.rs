/* Temp REST API documentation

Make a new transaction
  send, POST: {TRANSACTION CONTENT}
  response: accept / deny

Get information about bike SN
  send, GET: {SN: "...."}
  response: {transaction0: {...}, transaction1: {...}}

*/

use crate::blockchain::transaction::Transaction;
use serde_json::{Result, Value};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// main entry point for the REST server program
pub fn run_server() {
    rocket::ignite().mount("/", routes![index, new, info_id, info_addr]).launch();
}

#[get("/")]
fn index() -> &'static str {
    "https://github.com/SEP-G5/Backend/issues/4"
}

#[post("/new")]
fn new() -> &'static str {
    "todo"
}

#[get("/info_id/<id>", format = "json")]
fn info_id(id: String) -> String {
    let (t, sk) = Transaction::debug_make_register(id);
    serde_json::to_string(&t).expect("failed to convert to json")
}

#[derive(Serialize, Deserialize)]
struct Response {
    ok: bool,
    msg: String,
}

#[derive(Serialize, Deserialize)]
struct PubKey {
    pk: String
}

#[post("/info_pk", format = "json", data = "<data>")]
fn info_addr(data: String) -> String {

    let pk: PubKey = match serde_json::from_str(data.as_str()) {
        Ok(j) => j,
        Err(e) => {
        let r = Response{ok: false, msg: format!("{}", e)};
            return serde_json::to_string(&r).expect("failed to convert to json");
        },
    };

    // TODO ask the blockchain for all transactions with a given pk
    // ...

    let r = Response{ok: false, msg: format!("no transaction with that public key was found in the blockchain")};
    serde_json::to_string(&r).expect("failed to convert to json")
}
