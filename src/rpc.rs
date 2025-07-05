// src/rpc.rs

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use std::sync::{Arc, Mutex};
use crate::blockchain::{Blockchain, ChainMessage, Transaction};
use crate::mempool::Mempool;
use tokio::sync::mpsc;

// --- UBAH: Tambahkan Mempool dan kanal P2P ke AppState ---
struct AppState {
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mempool>,
    tx_p2p: mpsc::Sender<ChainMessage>,
}

#[get("/block_count")]
async fn get_block_count(data: web::Data<AppState>) -> impl Responder {
    let blockchain = data.blockchain.lock().unwrap();
    HttpResponse::Ok().json(blockchain.chain.len())
}

#[get("/block/{index}")]
async fn get_block_by_index(
    data: web::Data<AppState>,
    path: web::Path<u64>,
) -> impl Responder {
    let blockchain = data.blockchain.lock().unwrap();
    let index = path.into_inner();

    if let Some(block) = blockchain.chain.get(index as usize) {
        HttpResponse::Ok().json(block)
    } else {
        HttpResponse::NotFound().body(format!("Blok dengan index {} tidak ditemukan", index))
    }
}

// --- ENDPOINT BARU UNTUK MENERIMA TRANSAKSI ---
#[post("/transaction")]
async fn submit_transaction(
    data: web::Data<AppState>,
    tx: web::Json<Transaction>,
) -> impl Responder {
    let transaction = tx.into_inner();
    
    // Dapatkan akses ke state untuk validasi nonce
    let state = &data.blockchain.lock().unwrap().state;

    // Coba tambahkan transaksi ke mempool
    match data.mempool.add_transaction(transaction.clone(), state) {
        Ok(_) => {
            // Jika berhasil, kirim ke task P2P untuk disiarkan ke jaringan
            if let Err(e) = data.tx_p2p.send(ChainMessage::NewTransaction(transaction)).await {
                eprintln!("RPC: Gagal mengirim transaksi ke kanal P2P: {}", e);
                // Ini adalah kesalahan internal server
                return HttpResponse::InternalServerError().body("Gagal menyiarkan transaksi");
            }
            HttpResponse::Ok().json("Transaksi diterima dan disiarkan")
        }
        Err(e) => {
            // Jika gagal, kembalikan error ke klien
            HttpResponse::BadRequest().body(e.to_string())
        }
    }
}

// --- UBAH: Perbarui fungsi run ---
pub async fn run(
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mempool>,
    tx_p2p: mpsc::Sender<ChainMessage>,
    port: u16,
) -> std::io::Result<()> {
    println!("Menjalankan server RPC di http://127.0.0.1:{}", port);
    println!("Endpoint tersedia:");
    println!("  GET  /block_count");
    println!("  GET  /block/{{index}}");
    println!("  POST /transaction");

    // Buat instance AppState untuk di-share
    let app_data = web::Data::new(AppState {
        blockchain,
        mempool,
        tx_p2p,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .service(get_block_count)
            .service(get_block_by_index)
            .service(submit_transaction)
    })
    .bind(("127.0.0.1", port))?
    .run()
    .await
}