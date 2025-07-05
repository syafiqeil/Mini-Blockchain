// src/mempool.rs

use crate::blockchain::Transaction;
use crate::state::StateMachine;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

// Mempool akan menjadi struct yang thread-safe
#[derive(Clone)]
pub struct Mempool {
    transactions: Arc<Mutex<HashSet<Transaction>>>,
}

impl Mempool {
    /// Membuat instance Mempool baru yang kosong.
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Menambahkan transaksi ke mempool setelah validasi dasar.
    ///
    /// Validasi di sini hanya mencakup tanda tangan dan nonce.
    /// Saldo tidak diperiksa di sini karena bisa berubah sebelum transaksi dimasukkan ke blok.
    pub fn add_transaction(&self, tx: Transaction, state: &StateMachine) -> Result<(), &'static str> {
        // 1. Verifikasi tanda tangan transaksi
        if !tx.verify() {
            return Err("Tanda tangan tidak valid");
        }

        // 2. Verifikasi nonce
        let sender_account = state.get_account(&tx.sender)
            .map_err(|_| "Gagal akses database")?
            .ok_or("Akun pengirim tidak ditemukan")?;

        if tx.nonce < sender_account.nonce {
            return Err("Nonce sudah usang (replay attack?)");
        }

        // Jika semua validasi lolos, tambahkan ke set
        let mut pool = self.transactions.lock().unwrap();
        if pool.insert(tx) {
            println!("MEMPOOL: Transaksi baru ditambahkan. Total di mempool: {}", pool.len());
            Ok(())
        } else {
            Err("Transaksi sudah ada di mempool")
        }
    }

    /// Mengambil sejumlah transaksi dari pool untuk dimasukkan ke blok.
    /// Transaksi yang diambil juga akan dihapus dari pool.
    pub fn get_transactions(&self, count: usize) -> Vec<Transaction> {
        let mut pool = self.transactions.lock().unwrap();
        
        // Ambil 'count' transaksi pertama, atau semua jika lebih sedikit.
        let transactions_to_take: Vec<Transaction> = pool.iter().take(count).cloned().collect();

        // Hapus transaksi yang sudah diambil dari pool
        for tx in &transactions_to_take {
            pool.remove(tx);
        }
        
        if !transactions_to_take.is_empty() {
             println!("MEMPOOL: Mengambil {} transaksi untuk blok baru.", transactions_to_take.len());
        }

        transactions_to_take
    }

    /// Fungsi untuk menambahkan transaksi yang diterima dari jaringan P2P
    /// Validasi lebih sederhana karena kita percaya peer sudah memvalidasi nonce.
    pub fn add_from_p2p(&self, tx: Transaction) {
        if tx.verify() {
            let mut pool = self.transactions.lock().unwrap();
            pool.insert(tx);
        }
    }
}