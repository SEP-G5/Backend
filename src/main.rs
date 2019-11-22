#![feature(proc_macro_hygiene, decl_macro)]

mod backend;
mod blockchain;
mod rest;

// ========================================================================== //

use backend::Backend;

// ========================================================================== //

fn main() {
    let backend = Backend::new();
    backend.run();

    /*
    // Launch REST server
    let (tx, rx) = mpsc::channel();
    //rest::server::run_server();

    // On REST thread
    let (to, mut ro) = oneshot::channel();
    let op0 = Operation::QueryPeers { result: to };

    // Happens on other thread...
    thread::spawn(move || loop {
        match rx.recv() {
            Ok(op) => match op {
                Operation::QueryPeers { result: r } => {
                    let mut p = Peers { peers: Vec::new() };
                    p.peers.push(Peer {
                        ip: format!("123.123.123.123"),
                    });
                    p.peers.push(Peer {
                        ip: format!("1.2.3.4"),
                    });
                    r.send(p).expect("Aha!");
                }
                _ => {}
            },
            Err(e) => {
                panic!("Failed on other thread: {}", e);
            }
        }
    });

    // Send to Server
    tx.send(op0).expect("Failed to send");

    // Back on REST thread
    let mut failed_attempts = 0;
    'cool: loop {
        let res = ro.try_recv();
        match res {
            Ok(o) => {
                if let Some(t) = o {
                    println!("Peers: {:?}", t);
                    break 'cool;
                } else {
                    failed_attempts += 1;
                }
            }
            Err(e) => {
                println!("Operation cancelled: {}", e);
                panic!();
            }
        }
    }

    println!("Finally done after {} attempts", failed_attempts);*/
}
