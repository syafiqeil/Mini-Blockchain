// src/blockchain.rs

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

// Menggunakan kembali definisi dari fase-fase sebelumnya
// Pastikan semua yang diimpor di sini bersifat publik di modul asalnya
use crate::state::{Address, StateMachine};
use crate::crypto::{KeyPair, PUBLIC_KEY_SIZE, SIGNATURE_SIZE};
use crate::crypto;

// Definisikan alias tipe untuk membuat kode lebih mudah dibaca
pub type PublicKey = [u8; PUBLIC_KEY_SIZE];
pub type Signature = [u8; SIGNATURE_SIZE];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    // Definisi transaksi tetap sama
    pub sender: Address,
    pub recipient: Address,
    pub amount: u64,
    pub nonce: u64,
    #[serde(with = "serde_bytes")]
    pub signature: Signature,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub index: u64,
    pub timestamp: u128,
    pub prev_hash: Vec<u8>,
    pub hash: Vec<u8>,
    pub transactions: Vec<Transaction>,
    #[serde(with = "serde_bytes")]
    pub signature: Signature, // Tanda tangan dari authority
    pub authority: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChainMessage { // <-- TAMBAHKAN `pub` DI SINI
    NewBlock(Block),
}

// Implementasi untuk Block
impl Block {
    /// Membuat Genesis Block (blok pertama dalam chain)
    /// Membuat Genesis Block (blok pertama dalam chain)
    pub fn genesis() -> Self {
        let mut block = Block {
            index: 0,
            // Gunakan nilai konstan, bukan waktu saat ini
            timestamp: 1704067200000, // Contoh: 1 Januari 2024 00:00:00 GMT
            prev_hash: vec![0; 32],
            hash: Vec::new(),
            transactions: vec![],
            signature: [0; SIGNATURE_SIZE],
            authority: [0; PUBLIC_KEY_SIZE],
        };
        block.hash = Self::calculate_hash(&block); // Hitung hash berdasarkan data konstan
        block
    }

    /// Menghitung hash dari sebuah blok
    pub fn calculate_hash(block: &Block) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&block.index.to_be_bytes());
        data.extend_from_slice(&block.timestamp.to_be_bytes());
        data.extend_from_slice(&block.prev_hash);
        data.extend_from_slice(&block.authority);
        // Di dunia nyata, kita juga akan menyertakan hash dari semua transaksi (Merkle Root)

        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

pub struct Blockchain {
    pub chain: Vec<Block>,
    pub state: StateMachine, // Menyimpan state machine untuk interaksi database
}

// Implementasi untuk Blockchain
impl Blockchain {
    /// Membuat instance Blockchain baru dengan Genesis Block
    pub fn new(db_path: &str) -> Self {
        let state = StateMachine::new(db_path).expect("Gagal membuka database state");
        Self {
            chain: vec![Block::genesis()],
            state,
        }
    }

    /// Membuat blok baru (belum ditambahkan ke chain)
    pub fn create_block(&self, authority_keypair: &KeyPair) -> Block {
        let last_block = self.chain.last().expect("Chain tidak boleh kosong");
        let new_index = last_block.index + 1;
        let new_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

        let mut new_block = Block {
            index: new_index,
            timestamp: new_timestamp,
            prev_hash: last_block.hash.clone(),
            hash: Vec::new(), // Akan dihitung setelahnya
            transactions: vec![], // Untuk saat ini, blok kosong
            signature: [0; SIGNATURE_SIZE], // Akan diisi
            authority: authority_keypair.public_key,
        };
        
        // Hitung hash dan tanda tangani blok
        let hash = Block::calculate_hash(&new_block);
        new_block.hash = hash.clone();
        new_block.signature = authority_keypair.sign(&hash).expect("Gagal menandatangani blok");

        new_block
    }

    /// Menambahkan blok baru ke dalam chain setelah validasi
    pub fn add_block(&mut self, block: Block) -> bool {
        let last_block = self.chain.last().unwrap();

        // 1. Validasi Index
        if block.index != last_block.index + 1 {
            eprintln!("Validasi Gagal: Index tidak valid");
            return false;
        }
        // 2. Validasi Previous Hash
        if block.prev_hash != last_block.hash {
            eprintln!("Validasi Gagal: Previous hash tidak cocok");
            return false;
        }
        
        // 3. Validasi Hash Blok
        let calculated_hash = Block::calculate_hash(&block);
        if block.hash != calculated_hash {
            eprintln!("Validasi Gagal: Hash blok tidak valid");
            return false;
        }

        // 4. Validasi Tanda Tangan Authority
        if !crypto::verify(&block.authority, &block.hash, &block.signature) {
            eprintln!("Validasi Gagal: Tanda tangan authority tidak valid");
            return false;
        }

        println!("Blok baru divalidasi dan ditambahkan ke chain: index {}", block.index);
        self.chain.push(block);
        true
    }
}