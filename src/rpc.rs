// src/rpc.rs

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::blockchain::{Blockchain, ChainMessage, Transaction};
use crate::mempool::Mempool;

use log::{info, error, warn};

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

#[post("/transaction")]
async fn submit_transaction(
    data: web::Data<AppState>,
    tx: web::Json<Transaction>,
) -> impl Responder {
    let transaction = tx.into_inner();
    
    let state = &data.blockchain.lock().unwrap().state;

    match data.mempool.add_transaction(transaction.clone(), state) {
        Ok(_) => {
            info!("RPC: Menerima transaksi valid, menyiarkan ke P2P.");
            if let Err(e) = data.tx_p2p.send(ChainMessage::NewTransaction(transaction)).await {
                error!("Gagal mengirim transaksi ke kanal P2P: {}", e);
                return HttpResponse::InternalServerError().body("Gagal menyiarkan transaksi");
            }
            HttpResponse::Ok().json("Transaksi diterima dan disiarkan")
        }
        Err(e) => {
            warn!("RPC: Menerima transaksi tidak valid: {}", e);
            HttpResponse::BadRequest().body(e.to_string())
        }
    }
}

pub async fn run(
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mempool>,
    tx_p2p: mpsc::Sender<ChainMessage>,
    port: u16,
) -> std::io::Result<()> {
    let server_addr = format!("127.0.0.1:{}", port);
    info!("Menjalankan server RPC di http://{}", server_addr);
    info!("Endpoint tersedia: GET /block_count, GET /block/{{index}}, POST /transaction");

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
    .bind(server_addr)?
    .run()
    .await
}
