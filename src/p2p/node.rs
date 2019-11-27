use crate::p2p::{network::Rx, packet::Packet, shared::Shared,
                 shared::B2NTx};
use bincode;
use bytes::{BufMut, Bytes, BytesMut};
use futures::future::{self, Either};
use futures::try_ready;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use tokio::io;
use tokio::net::TcpStream;
use tokio::prelude::*;

pub struct Node {
    addr: SocketAddr,
    con: Connection,
    state: Arc<Mutex<Shared>>,

    /// Receive half of the backend-to-network channel
    b2n_rx: Rx,
}

impl Node {
    pub fn new(con: Connection, state: Arc<Mutex<Shared>>) -> Node {
        let (b2n_tx, b2n_rx) = mpsc::channel();

        let node = Node {
            addr: con.stream.peer_addr().expect("failed to get peer address"),
            con,
            state,
            b2n_rx,
        };

        {
            let mut l = node.state.lock().expect("failed to aquire lock");
            l.b2n_tx.insert(node.addr.clone(), B2NTx::new(b2n_tx, task::current()));
        }

        node
    }

    pub fn get_addr(&self) -> &SocketAddr {
        &self.addr
    }

    fn on_packet(&mut self) {
        println!("got packet from {:?}", self.addr);

        let packet: Packet = match bincode::deserialize(&self.con.get_rd()) {
            Ok(p) => p,
            Err(e) => {
                println!("failed to deserialize packet [{:?}]", e);
                return;
            }
        };

        let l = self.state.lock().expect("failed to aquire lock");
        if let Err(e) = l.n2b_tx.send(packet) {
            println!("failed to send packet on n2b_tx channel [{:?}]", e);
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.state.lock().unwrap().b2n_tx.remove(&self.addr);
    }
}

impl Future for Node {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        println!("poll (impl Future for Node)");
        // Read messages from backend.
        if let Ok(packet) = self.b2n_rx.try_recv() {
            println!("got packet from backend");
            if let Err(_) = self.con.send(&packet) {
                return Ok(Async::Ready(()));
            }
        }

        // We have a for loop such that it can handle many backend
        // messages per poll. Then we limit it to a maximum via the
        // MAX_POLLS constant, such that it does not use too much time.
        /*
        const MAX_POLLS: usize = 10;
        for i in 0..MAX_POLLS {
            if let Ok(packet) = self.b2n_rx.try_recv() {
                if let Err(_) = self.con.send(&packet) {
                    return Ok(Async::Ready(()));
                }
            } else {
                break;
            }
        }
*/

        // Read messages from other nodes
        while let Async::Ready(can_read) = self.con.poll()? {
            match can_read {
                Some(true) => {
                    self.on_packet();
                    break;
                }
                Some(false) => {
                    break;
                }
                None => {
                    println!("READY");
                    return Ok(Async::Ready(()));
                }
            }
        }

        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
pub struct Connection {
    pub stream: TcpStream,
    pub rd: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        let mut con = Connection {
            stream,
            rd: BytesMut::new(),
        };
        con.rd.reserve(1024 * 1024);
        con
    }

    pub fn send(&mut self, packet: &Packet) -> Result<(), ()> {
        println!("sending {:?}", packet);
        let bin = match bincode::serialize(packet) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("failed to serialize packet [{:?}], dropping connection", e);
                return Err(());
            },
        };
        match self.stream.write_all(&bin[..]) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    pub fn get_rd(&self) -> &BytesMut {
        &self.rd
    }

    pub fn fill_read_buf(&mut self) -> Poll<(), io::Error> {
        self.rd.clear();
        loop {
            let n = try_ready!(self.stream.read_buf(&mut self.rd));
            if n == 0 {
                return Ok(Async::Ready(()));
            }
        }
    }
}

impl Stream for Connection {
    type Item = bool;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let stream_closed = self.fill_read_buf()?.is_ready();

        if stream_closed {
            return Ok(Async::Ready(None));
        } else {
            if self.rd.len() > 0 {
                return Ok(Async::Ready(Some(true)));
            } else {
                return Ok(Async::Ready(Some(false)));
            }
        }
    }
}
