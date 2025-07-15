// src/main.rs

use clap::Parser;
use evice_blockchain::{
    blockchain::{Blockchain, ChainMessage},
    crypto, mempool::Mempool, p2p, rpc,
};
use log::{error, info};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

const MAX_TRANSACTIONS_PER_BLOCK: usize = 10;

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    #[clap(long)]
    is_authority: bool,
    #[clap(long, default_value = "./database")]
    db_path: String,
    #[clap(long)]
    bootstrap: bool,
    #[clap(long, default_value = "8080")]
    rpc_port: u16,
    #[clap(long)]
    bootstrap_node: Option<String>,
    #[clap(long, default_value = "50000")]
    p2p_port: u16,
}

#[tokio::main]
async fn main() {
    LogTracer::init().unwrap();

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Gagal meng-set subscriber logging");

    let args = Args::parse();

    let blockchain = Arc::new(Mutex::new(Blockchain::new(&args.db_path)));

    if args.bootstrap {
        info!("Mem-bootstrap state awal dengan akun genesis...");
        blockchain.lock().unwrap().state.bootstrap_accounts();
        info!("Bootstrap selesai. Program berhenti.");
        return;
    }

    let mempool = Arc::new(Mempool::new());
    let (tx_p2p, rx_p2p) = mpsc::channel::<ChainMessage>(100);

    if args.is_authority {
        let authority_keypair = Arc::new(crypto::KeyPair::new());
        info!("Menjalankan sebagai NODE OTORITAS.");
        info!(
        "Alamat Otoritas: 0x{}",
        hex::encode(authority_keypair.public_key_bytes())
        );
        
        let chain_clone = Arc::clone(&blockchain);
        let key_clone = Arc::clone(&authority_keypair);
        let mempool_clone = Arc::clone(&mempool);
        let tx_p2p_clone_auth = tx_p2p.clone();

        tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(10));
            loop {
                tick.tick().await;
                let transactions = mempool_clone.get_transactions(MAX_TRANSACTIONS_PER_BLOCK);
                if !transactions.is_empty() {
                    let new_block = {
                        let mut chain = chain_clone.lock().unwrap();
                        let block = chain.create_block(&key_clone, transactions);
                        info!(
                            "OTORITAS: Membuat blok baru #{} dengan {} transaksi.",
                            block.index,
                            block.transactions.len()
                        );
                        if chain.add_block(block.clone()) {
                            block
                        } else {
                            error!("OTORITAS: Gagal menambahkan blok yang baru dibuat ke chain lokal.");
                            continue;
                        }
                    };
                    if tx_p2p_clone_auth.send(ChainMessage::NewBlock(new_block)).await.is_err() {
                        break;
                    }
                }
            }
        });
    } else {
        info!("Menjalankan sebagai NODE REGULER.");
    }

    // Jalankan P2P di background task
    let p2p_blockchain_clone = Arc::clone(&blockchain);
    let p2p_mempool_clone = Arc::clone(&mempool);
    let bootstrap_node_clone = args.bootstrap_node.clone();
    let p2p_port = args.p2p_port;
    tokio::spawn(async move {
        info!("--- Menjalankan Jaringan P2P & Konsensus ---");
        if let Err(e) = p2p::run(p2p_blockchain_clone, p2p_mempool_clone, rx_p2p, bootstrap_node_clone, p2p_port).await {
            error!("Error P2P Runtime: {}", e);
        }
    });

    // Jalankan RPC di foreground (main task).
    // Ini akan menjaga program tetap berjalan dan memegang 'tx_p2p' yang asli.
    info!("--- Menjalankan Server RPC ---");
    if let Err(e) = rpc::run(blockchain, mempool, tx_p2p, args.rpc_port).await {
        error!("Error server RPC: {}", e);
    }
}
