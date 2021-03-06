use crate::p2p::{
    network::Rx,
    packet::{Packet, PacketCodec, PacketErr},
    shared::Shared,
};
use futures::{SinkExt, Stream, StreamExt};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio_util::codec::Framed;

pub enum PacketFrom {
    P2P(Packet, SocketAddr),
    Backend(Packet),
}

pub struct Node {
    addr: SocketAddr,
    state: Arc<Mutex<Shared>>,
    packets: Framed<TcpStream, PacketCodec>,
    /// Receive half of the backend-to-network channel
    b2n_rx: Rx,
}

impl Node {
    pub async fn new(stream: TcpStream, state: Arc<Mutex<Shared>>) -> Node {
        // TODO what size value to give mpsc channel
        let (b2n_tx, b2n_rx) = mpsc::channel(1337);
        let addr = stream.peer_addr().expect("failed to get peer address");
        state.lock().await.b2n_tx.insert(addr, b2n_tx);
        let node = Node {
            addr,
            state,
            packets: Framed::new(stream, PacketCodec::new()),
            b2n_rx,
        };
        node
    }

    pub fn get_addr(&self) -> &SocketAddr {
        &self.addr
    }

    /// Will be run for the entire lifetime of the node. When this extits
    /// the connection is closed.
    pub async fn run(&mut self) {
        {
            let state = &mut self.state.lock().await;
            println!(
                "new node connected on [{:?}], {} active nodes",
                self.get_addr(),
                state.b2n_tx.len()
            );
            /*
            print!("== == == nodes");
            for (addr, _) in &state.b2n_tx {
                print!("\n\t{}", addr);
            }
            println!("");
             */
        }

        while let Some(res) = self.next().await {
            match res {
                // process messages from the remote node
                Ok(PacketFrom::P2P(packet, addr)) => {
                    //println!("\npacket from p2p ('{:?}')", packet);
                    match self
                        .state
                        .lock()
                        .await
                        .n2b_tx
                        .send((packet, Some(addr)))
                        .await
                    {
                        Ok(_) => (),
                        Err(_) => println!("failed to receive from p2p [{}]", addr),
                    }
                }
                // process messages from backend
                Ok(PacketFrom::Backend(packet)) => {
                    //println!("packet from backend");
                    match packet {
                        Packet::CloseConnection() => {
                            println!("node {} got CloseConnection packet", self.addr);
                            break;
                        }
                        packet => match self.packets.send(packet).await {
                            Ok(_) => {}
                            Err(e) => println!("failed to send to node [{:?}]", e),
                        },
                    }
                }
                Err(e) => {
                    println!(
                        "error when processing on node [{:?}] with [{:?}]",
                        self.get_addr(),
                        e
                    );
                }
            }
        }

        {
            let state = &mut self.state.lock().await;
            println!(
                "node [{:?}] disconnected, {} remaining",
                self.get_addr(),
                state.b2n_tx.len() - 1
            );
            state.b2n_tx.remove(&self.addr);
        }
    }
}

impl Stream for Node {
    type Item = Result<PacketFrom, PacketErr>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // poll backend-to-nextwork channel
        if let Poll::Ready(Some((packet, _))) = self.b2n_rx.poll_next_unpin(cx) {
            return Poll::Ready(Some(Ok(PacketFrom::Backend(packet))));
        }

        // poll network stream
        let res: Option<_> = futures::ready!(self.packets.poll_next_unpin(cx));
        Poll::Ready(match res {
            Some(Ok(packet)) => Some(Ok(PacketFrom::P2P(packet, self.addr))),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        })
    }
}
