// src/main.rs

// Deklarasi modul yang ada
mod crypto;
mod state;
mod p2p;
mod blockchain;
mod rpc;

// Impor yang dibutuhkan
use clap::Parser;
use std::sync::{ Arc, Mutex };
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use crate::blockchain::Blockchain;

/// Program Blockchain P2P Sederhana
#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    #[clap(long)]
    is_authority: bool,
    #[clap(long, default_value = "./database")]
    db_path: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let blockchain = Arc::new(Mutex::new(Blockchain::new(&args.db_path)));

    // Buat channel untuk komunikasi antar-task
    let (tx, rx) = mpsc::channel(32);

    // --- Task Otoritas ---
    if args.is_authority {
        let authority_keypair = Arc::new(crypto::KeyPair::new().unwrap());
        println!("Menjalankan sebagai NODE OTORITAS.");
        println!("Alamat Otoritas: 0x{}", hex::encode(authority_keypair.public_key));

        let chain_clone = Arc::clone(&blockchain);
        let key_clone = Arc::clone(&authority_keypair); // <-- PASTIKAN BARIS INI ADA
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let mut tick = interval(Duration::from_secs(10));
            loop {
                // DI DALAM main.rs, di dalam loop tokio::spawn untuk authority
                tick.tick().await;
                let new_block = {
                    // Kunci Mutex untuk mendapatkan akses tulis ke blockchain
                    let mut chain = chain_clone.lock().unwrap();

                    // Buat blok baru
                    let block = chain.create_block(&key_clone);
                    println!("OTORITAS: Menambahkan blok baru #{} ke chain lokal.", block.index);

                    // PENTING: Otoritas menambahkan blok ke chain-nya sendiri
                    chain.add_block(block.clone());

                    // Kembalikan blok untuk dikirim ke jaringan
                    block
                };
                println!("OTORITAS: Membuat blok baru #{}.", new_block.index);
                if tx.send(new_block).await.is_err() {
                    eprintln!("Channel receiver ditutup, menghentikan produksi blok.");
                    break;
                }
            }
        });
    } else {
        println!("Menjalankan sebagai NODE REGULER.");
    }

    // --- Task Jaringan P2P ---
    let p2p_blockchain_clone = Arc::clone(&blockchain);
    tokio::spawn(async move {
        println!("--- Menjalankan Jaringan P2P & Konsensus ---");
        if let Err(e) = p2p::run(p2p_blockchain_clone, rx).await {
            eprintln!("Error P2P Runtime: {}", e);
        }
    });

    // --- Server RPC (berjalan di main thread) ---
    println!("--- Menjalankan Server RPC ---");
    if let Err(e) = rpc::run(blockchain).await {
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
