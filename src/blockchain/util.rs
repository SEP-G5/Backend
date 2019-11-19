use std::time::SystemTime;

pub type Timestamp = u64;

pub fn make_timestamp() -> Timestamp {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("failed to make timestamp");
    return ts.as_secs() as Timestamp;
}
