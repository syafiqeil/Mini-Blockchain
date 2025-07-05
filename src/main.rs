// src/main.rs

use clap::Parser;
use std::sync::{ Arc, Mutex };
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;

// Impor semua yang dibutuhkan dari library `evice_blockchain` Anda
use evice_blockchain::blockchain::{Blockchain, ChainMessage};
use evice_blockchain::crypto;
use evice_blockchain::mempool::Mempool;
use evice_blockchain::p2p;
use evice_blockchain::rpc;

// Jumlah transaksi maksimum yang akan dimasukkan dalam satu blok
const MAX_TRANSACTIONS_PER_BLOCK: usize = 10;

/// Program Blockchain P2P Sederhana
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
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let blockchain = Arc::new(Mutex::new(Blockchain::new(&args.db_path)));

    // Jika flag --bootstrap dijalankan, buat akun awal lalu keluar.
    if args.bootstrap {
        println!("Mem-bootstrap state awal dengan akun genesis...");
        blockchain.lock().unwrap().state.bootstrap_accounts();
        println!("Bootstrap selesai. Program berhenti.");
        return;
    }
    
    // --- BUAT INSTANCE MEMPOOL ---
    let mempool = Arc::new(Mempool::new());

    // --- UBAH KANAL UNTUK MENGIRIM ChainMessage ---
    let (tx_p2p, rx_p2p) = mpsc::channel::<ChainMessage>(100);

    // --- Task Otoritas ---
    if args.is_authority {
        let authority_keypair = Arc::new(crypto::KeyPair::new().unwrap());
        println!("Menjalankan sebagai NODE OTORITAS.");
        println!("Alamat Otoritas: 0x{}", hex::encode(authority_keypair.public_key));

        let chain_clone = Arc::clone(&blockchain);
        let key_clone = Arc::clone(&authority_keypair);
        let mempool_clone = Arc::clone(&mempool);
        let tx_p2p_clone = tx_p2p.clone();

        tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(10));
            loop {
                tick.tick().await;

                // --- LOGIKA BARU: AMBIL TRANSAKSI DARI MEMPOOL ---
                let transactions = mempool_clone.get_transactions(MAX_TRANSACTIONS_PER_BLOCK);
                
                // Hanya buat blok jika ada transaksi atau secara periodik (opsional)
                // Untuk kesederhanaan, kita akan selalu membuat blok, meskipun kosong.
                
                let new_block = {
                    let mut chain = chain_clone.lock().unwrap();
                    
                    // --- UBAH: Masukkan transaksi ke dalam blok ---
                    let block = chain.create_block(&key_clone, transactions);
                    println!("OTORITAS: Membuat blok baru #{} dengan {} transaksi.", block.index, block.transactions.len());

                    // Otoritas menambahkan blok ke chain-nya sendiri
                    // Logika add_block sekarang juga memproses transaksi di dalamnya
                    if chain.add_block(block.clone()) {
                        block
                    } else {
                        // Jika blok tidak valid, lewati siklus ini
                        eprintln!("OTORITAS: Gagal menambahkan blok yang baru dibuat ke chain lokal.");
                        continue;
                    }
                };

                // Kirim blok baru ke task P2P untuk disiarkan
                if tx_p2p_clone.send(ChainMessage::NewBlock(new_block)).await.is_err() {
                    eprintln!("Channel P2P ditutup, menghentikan produksi blok.");
                    break;
                }
            }
        });
    } else {
        println!("Menjalankan sebagai NODE REGULER.");
    }

    // --- Task Jaringan P2P ---
    let p2p_blockchain_clone = Arc::clone(&blockchain);
    let p2p_mempool_clone = Arc::clone(&mempool);
    tokio::spawn(async move {
        println!("--- Menjalankan Jaringan P2P & Konsensus ---");
        if let Err(e) = p2p::run(p2p_blockchain_clone, p2p_mempool_clone, rx_p2p).await {
            eprintln!("Error P2P Runtime: {}", e);
        }
    });

    // --- Server RPC (berjalan di main thread) ---
    println!("--- Menjalankan Server RPC ---");
    // --- UBAH: Berikan mempool dan kanal P2P ke RPC ---
    if let Err(e) = rpc::run(blockchain, mempool, tx_p2p, args.rpc_port).await {
        eprintln!("Error server RPC: {}", e);
    }
}

// Blok tes bisa tetap ada
#[cfg(test)]
mod tests {
    use super::crypto;

    #[test]
    fn test_full_crypto_flow() {
        let keypair = crypto::KeyPair::new().expect("Gagal membuat keypair");
        let message = b"Ini adalah pesan rahasia untuk Evice Blockchain";
        let signature = keypair.sign(message).expect("Gagal menandatangani pesan");
        let is_valid = crypto::verify(&keypair.public_key, message, &signature);
        assert!(is_valid, "Verifikasi tanda tangan seharusnya berhasil!");
    }
}
