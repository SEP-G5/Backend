use bytes::{BufMut, Bytes, BytesMut};
use futures::future::{self, Either};
use futures::try_ready;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use tokio::io;
use tokio::net::{TcpStream};
use tokio::prelude::*;
use crate::p2p::{shared::Shared, network::Rx, packet::Packet};
use bincode;
use crate::backend::operation::Operation;

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
            addr: con
                .stream
                .peer_addr()
                .expect("failed to get peer address"),
            con,
            state,
            b2n_rx,
        };

        {
            let mut l = node.state.lock().expect("failed to aquire lock");
            l.b2n_tx.insert(node.addr.clone(), b2n_tx);
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
            },
        };

        let l = self.state.lock().expect("failed to aquire lock");
        if let Err(e) = l.n2b_tx.send(Operation::GotPacket{packet}) {
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

        // Read messages from backend.
        // We have a for loop such that it can handle many backend
        // messages per poll. Then we limit it to a maximum via the
        // MAX_POLLS constant, such that it does not use too much time.
        const MAX_POLLS: usize = 10;
        for i in 0..MAX_POLLS {
            if let Ok(op) = self.b2n_rx.try_recv() {

            } else {
                break;
            }
        }

        // Read messages from other nodes
        while let Async::Ready(can_read) = self.con.poll()? {

            match can_read {
                Some(true) => {
                    self.on_packet();
                },
                Some(false) => {
                    return Ok(Async::NotReady);
                },
                None => {
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
    pub wr: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        let mut con = Connection { stream, rd: BytesMut::new(), wr: BytesMut::new() };
        con.rd.reserve(1024*1024);
        con.wr.reserve(1024*1024);
        con
    }

    pub fn get_rd(&self) -> &BytesMut {
        &self.rd
    }

    pub fn get_wr(&self) -> &BytesMut {
        &self.wr
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
