// src/p2p.rs

use libp2p::{
    futures::StreamExt,
    gossipsub, identity, kad, noise, request_response,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol,
};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::iter;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{select, sync::mpsc};

use crate::blockchain::{Block, Blockchain, ChainMessage};
use crate::mempool::Mempool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRequest {
    GetBlocks { since_index: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    Blocks { blocks: Vec<Block> },
}

const SYNC_PROTOCOL: StreamProtocol = StreamProtocol::new("/evice-blockchain/sync/1.0");

#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub req_resp: request_response::cbor::Behaviour<SyncRequest, SyncResponse>,
}

pub async fn run(
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mempool>,
    mut rx: mpsc::Receiver<ChainMessage>,
    bootstrap_node: Option<String>,
    p2p_port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    info!("Peer ID lokal: {}", local_peer_id);

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
        .with_behaviour(|key| {
            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub::Config::default(),
            )?;
            let store = kad::store::MemoryStore::new(local_peer_id);
            let kademlia = kad::Behaviour::new(local_peer_id, store);
            let req_resp = request_response::cbor::Behaviour::new(
                iter::once((SYNC_PROTOCOL, request_response::ProtocolSupport::Full)),
                request_response::Config::default(),
            );
            Ok(AppBehaviour {
                gossipsub,
                kademlia,
                req_resp,
            })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    let topic = gossipsub::IdentTopic::new("evice-blockchain-topic");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    if let Some(addr_str) = bootstrap_node {
        let remote_addr = Multiaddr::from_str(&addr_str)?;
        if let Some(remote_peer_id) = remote_addr.iter().last().and_then(|proto| {
            if let libp2p::multiaddr::Protocol::P2p(peer_id) = proto {
                Some(peer_id)
            } else {
                None
            }
        }) {
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&remote_peer_id, remote_addr.clone());
            info!("Menambahkan bootstrap node: {}", remote_addr);
        } else {
            return Err("Alamat bootstrap node tidak mengandung PeerId yang valid.".into());
        }
    }

    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
        warn!("P2P: Gagal memulai Kademlia bootstrap: {:?}", e);
    }      
    
    // --- PERBAIKAN UNTUK LINUX: Gunakan 0.0.0.0 ---
    // Ini memungkinkan node untuk menerima koneksi dari mesin lain.
    let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", p2p_port).parse()?;
    swarm.listen_on(listen_addr)?;

    loop {
        select! {
            Some(message_to_broadcast) = rx.recv() => {
                let json = serde_json::to_string(&message_to_broadcast)?;
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), json.as_bytes()) {
                    warn!("P2P: Gagal menyiarkan pesan: {:?}", e);
                }
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("Node mendengarkan di: {}/p2p/{}", address, local_peer_id);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        info!("Koneksi berhasil dibuat dengan peer: {}", peer_id);
                        let current_index = blockchain.lock().unwrap().chain.last().unwrap().index;
                        swarm.behaviour_mut().req_resp.send_request(&peer_id, SyncRequest::GetBlocks { since_index: current_index });
                        info!("SYNC: Mengirim permintaan GetBlocks ke peer {}", peer_id);
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Kademlia(event)) => {
                        info!("[KAD] Event: {:?}", event);
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                         match serde_json::from_slice::<ChainMessage>(&message.data) {
                            Ok(ChainMessage::NewBlock(block)) => {
                                info!("P2P: Menerima blok baru #{} dari jaringan via Gossip.", block.index);
                                let mut chain = blockchain.lock().unwrap();
                                if block.index > chain.chain.last().unwrap().index {
                                     chain.add_block(block);
                                }
                            }
                            Ok(ChainMessage::NewTransaction(tx)) => {
                                info!("P2P: Menerima transaksi baru dari jaringan via Gossip.");
                                mempool.add_from_p2p(tx);
                            }
                            Err(e) => {
                                error!("Gagal deserialisasi pesan Gossip: {}", e);
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
                                            error!("SYNC: Gagal mengirim response");
                                        }
                                    }
                                    request_response::Message::Response { response, .. } => {
                                        let SyncResponse::Blocks { blocks } = response;
                                        if blocks.is_empty() {
                                            info!("SYNC: Peer tidak memiliki blok baru. Chain sudah up-to-date.");
                                        } else {
                                            info!("SYNC: Menerima {} blok dari peer.", blocks.len());
                                            let mut chain = blockchain.lock().unwrap();
                                            for block in blocks {
                                                if block.index > chain.chain.last().unwrap().index {
                                                    chain.add_block(block);
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
