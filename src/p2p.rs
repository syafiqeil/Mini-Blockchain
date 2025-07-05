// src/p2p.rs

use libp2p::{
    futures::StreamExt,
    gossipsub,
    identity,
    mdns,
    noise,
    request_response,
    swarm::{ NetworkBehaviour, SwarmEvent },
    tcp,
    yamux,
    PeerId,
    StreamProtocol,
};

use std::iter;
use std::sync::{ Arc, Mutex };
use std::time::Duration;
use tokio::select;
use tokio::sync::mpsc;

use crate::blockchain::{ Block, Blockchain, ChainMessage };
use crate::mempool::Mempool;
use serde::{ Deserialize, Serialize };

// --- DEFINISI PESAN ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRequest {
    GetBlocks {
        since_index: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    Blocks {
        blocks: Vec<Block>,
    },
}

// --- PROTOKOL & CODEC ---
const SYNC_PROTOCOL: StreamProtocol = StreamProtocol::new("/evice-blockchain/sync/1.0");

// --- NETWORK BEHAVIOUR ---
#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub req_resp: request_response::cbor::Behaviour<SyncRequest, SyncResponse>,
}

// --- FUNGSI UTAMA P2P ---
pub async fn run(
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mempool>,
    mut rx: mpsc::Receiver<ChainMessage>
) -> Result<(), Box<dyn std::error::Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Peer ID lokal: {}", local_peer_id);

    let mut swarm = libp2p::SwarmBuilder
        ::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
        .with_behaviour(|key| {
            let gossipsub = gossipsub::Behaviour
                ::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub::Config::default()
                )
                .expect("Correct configuration");

            let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();

            let req_resp = request_response::cbor::Behaviour::new(
                iter::once((SYNC_PROTOCOL, request_response::ProtocolSupport::Full)),
                request_response::Config::default()
            );

            Ok(AppBehaviour {
                gossipsub,
                mdns,
                req_resp,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let topic = gossipsub::IdentTopic::new("evice-blockchain-topic");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    loop {
        select! {
            Some(message_to_broadcast) = rx.recv() => {
                let json = serde_json::to_string(&message_to_broadcast)?;
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), json.as_bytes()) {
                    eprintln!("P2P: Gagal menyiarkan pesan: {:?}", e);
                }
            }

            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Node mendengarkan di: {}", address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        println!("Koneksi berhasil dibuat dengan peer: {}", peer_id);
                        let current_index = blockchain.lock().unwrap().chain.last().unwrap().index;
                        swarm.behaviour_mut().req_resp.send_request(&peer_id, SyncRequest::GetBlocks { since_index: current_index });
                        println!("SYNC: Mengirim permintaan GetBlocks ke peer {}", peer_id);
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Mdns(event)) => {
                        match event {
                            mdns::Event::Discovered(list) => {
                                for (peer_id, _multiaddr) in list {
                                    swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                    swarm.add_peer_address(peer_id, _multiaddr.clone());
                                }
                            }
                            mdns::Event::Expired(list) => {
                                for (peer_id, _multiaddr) in list {
                                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                         match serde_json::from_slice::<ChainMessage>(&message.data) {
                            Ok(ChainMessage::NewBlock(block)) => {
                                println!("P2P: Menerima blok baru #{} dari jaringan via Gossip.", block.index);
                                let mut chain = blockchain.lock().unwrap();
                                if block.index == chain.chain.last().unwrap().index + 1 {
                                     chain.add_block(block);
                                } else {
                                    println!("SYNC: Blok Gossip #{} diterima, tapi tidak berurutan. Diabaikan.", block.index);
                                }
                            }
                            Ok(ChainMessage::NewTransaction(tx)) => {
                                println!("P2P: Menerima transaksi baru dari jaringan via Gossip.");
                                mempool.add_from_p2p(tx);
                            }
                            Err(e) => {
                                eprintln!("Gagal deserialisasi pesan Gossip: {}", e);
                            }
                        }
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::ReqResp(event)) => {
                        match event {
                            request_response::Event::Message { message, .. } => {
                                match message {
                                    request_response::Message::Request { request, channel, .. } => {
                                        let SyncRequest::GetBlocks { since_index } = request;
                                            let chain = blockchain.lock().unwrap();
                                            let blocks_to_send: Vec<Block> = chain.chain
                                                .iter()
                                                .skip(since_index as usize + 1)
                                                .cloned()
                                                .collect();
                                            if swarm.behaviour_mut().req_resp.send_response(channel, SyncResponse::Blocks { blocks: blocks_to_send }).is_err() {
                                                eprintln!("SYNC: Gagal mengirim response");
                                            }
                                        }
                                    
                                    request_response::Message::Response { response, .. } => {
                                        let SyncResponse::Blocks { blocks } = response;

                                        if blocks.is_empty() {
                                            println!("SYNC: Peer tidak memiliki blok baru. Chain sudah up-to-date.");
                                        } else {
                                            println!("SYNC: Menerima {} blok dari peer.", blocks.len());
                                            let mut chain = blockchain.lock().unwrap();
                                            for block in blocks {
                                                if block.index == chain.chain.last().unwrap().index + 1 {
                                                    chain.add_block(block);
                                                } else {
                                                    eprintln!("SYNC: Menerima blok tidak berurutan dari response ({} vs {}). Proses sinkronisasi dihentikan.", block.index, chain.chain.last().unwrap().index + 1);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
