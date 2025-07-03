// src/p2p.rs

use libp2p::{
    futures::StreamExt, // <-- TAMBAHKAN BARIS INI
    gossipsub,
    identity,
    mdns,
    noise,
    swarm::NetworkBehaviour,
    swarm::SwarmEvent,
    tcp,
    yamux,
    PeerId,
    Swarm,
};

// Tambahkan ini di atas
use crate::blockchain::Block;
use crate::blockchain::{ Blockchain, ChainMessage };
use serde::{ Serialize, Deserialize };
use std::collections::hash_map::DefaultHasher;
use std::hash::{ Hash, Hasher };
use std::sync::{ Arc, Mutex };
use std::time::Duration;
use tokio::io::{ self, AsyncBufReadExt };
use tokio::select; // Pastikan ChainMessage publik atau pindahkan
use tokio::sync::mpsc;

// Mendefinisikan 'perilaku' jaringan kita dengan menggabungkan
// protokol-protokol yang kita butuhkan.
#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

// Fungsi utama untuk menjalankan node P2P
pub async fn run(
    blockchain: Arc<Mutex<Blockchain>>,
    mut rx: mpsc::Receiver<Block>
) -> Result<(), Box<dyn std::error::Error>> {
    // Membuat identitas unik untuk node kita
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Peer ID lokal: {}", local_peer_id);

    // Konfigurasi untuk protokol gossipsub (broadcast pesan)
    let gossipsub_config = gossipsub::ConfigBuilder
        ::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(|message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        })
        .build()?;

    let mut gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config
    )?;

    // Membuat topik gossip. Semua node yang subscribe ke topik ini akan menerima pesan.
    let topic = gossipsub::IdentTopic::new("evice-blockchain-topic");
    gossipsub.subscribe(&topic)?;

    // Membuat swarm, yang merupakan inti dari networking libp2p
    let mut swarm = libp2p::SwarmBuilder
        ::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default)?
        .with_behaviour(|_key| {
            // MDNS untuk penemuan peer di jaringan lokal
            let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;
            Ok(AppBehaviour { gossipsub, mdns })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    // Mulai mendengarkan koneksi masuk dari alamat manapun di port yang acak
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Loop utama untuk memproses event jaringan dan input dari user
    loop {
        select! {
            Some(block) = rx.recv() => {
                println!("P2P: Menerima blok #{} dari producer, menyiarkan ke jaringan...", block.index);
                let message = ChainMessage::NewBlock(block);
                let json = serde_json::to_string(&message)?;
                if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), json.as_bytes()) {
                    eprintln!("Gagal menyiarkan blok: {:?}", e);
                }
            }

            // Menunggu event dari jaringan
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Node mendengarkan di: {}", address);
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Mdns(event)) => {
                        match event {
                            mdns::Event::Discovered(list) => {
                                for (peer_id, _multiaddr) in list {
                                    println!("MDNS menemukan peer baru: {}", peer_id);
                                    swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                }
                            }
                            mdns::Event::Expired(list) => {
                                for (peer_id, _multiaddr) in list {
                                    println!("MDNS peer kedaluwarsa: {}", peer_id);
                                    swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: _peer_id,
                        message_id: _id,
                        message,
                    })) => {
                        match serde_json::from_slice::<ChainMessage>(&message.data) {
                            Ok(ChainMessage::NewBlock(block)) => {
                                println!(
                                    "Menerima blok baru #{} dari peer: {:?}",
                                    block.index,
                                    message.source
                                );
                                // Di dalam SwarmEvent::Behaviour(AppBehaviourEvent::Gossipsub...)
                                if let Ok(ChainMessage::NewBlock(block)) = serde_json::from_slice::<ChainMessage>(&message.data) {
                                    println!("P2P: Menerima blok baru #{} dari jaringan.", block.index);
                                    let mut chain = blockchain.lock().unwrap();
                                    chain.add_block(block);
                                }
                            }
                            Err(e) => {
                                eprintln!("Gagal deserialisasi pesan: {}", e);
                            }
                        }
                        println!(
                            "Menerima pesan: '{}' dari peer: {:?}",
                            String::from_utf8_lossy(&message.data),
                            message.source
                        );
                    }
                     SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        println!("Koneksi berhasil dibuat dengan peer: {}", peer_id);
                    }
                    _ => {}
                }
            }
        }
    }
}
