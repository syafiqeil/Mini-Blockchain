// src/rpc.rs

use crate::blockchain::{Block, Blockchain};
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use std::sync::{Arc, Mutex};

// Wrapper untuk state blockchain kita agar bisa di-share dengan aman
// ke semua thread worker Actix.
type AppState = web::Data<Arc<Mutex<Blockchain>>>;

#[get("/block_count")]
async fn get_block_count(data: AppState) -> impl Responder {
    let blockchain = data.lock().unwrap();
    HttpResponse::Ok().json(blockchain.chain.len())
}

#[get("/block/{index}")]
async fn get_block_by_index(
    data: AppState,
    path: web::Path<u64>,
) -> impl Responder {
    let blockchain = data.lock().unwrap();
    let index = path.into_inner();

    if let Some(block) = blockchain.chain.get(index as usize) {
        HttpResponse::Ok().json(block)
    } else {
        HttpResponse::NotFound().body(format!("Blok dengan index {} tidak ditemukan", index))
    }
}

// Fungsi utama untuk menjalankan server RPC
pub async fn run(blockchain: Arc<Mutex<Blockchain>>) -> std::io::Result<()> {
    println!("Menjalankan server RPC di http://127.0.0.1:8080");

    // Membuat instance AppState untuk di-share
    let app_data = web::Data::new(blockchain);

    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone()) // Share state ke semua handler
            .service(get_block_count)
            .service(get_block_by_index)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}