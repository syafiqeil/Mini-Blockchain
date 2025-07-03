// src/state.rs

use rocksdb::{DB, Options};
use crate::crypto::{PUBLIC_KEY_SIZE}; // Gunakan konstanta dari modul crypto kita
use serde::{Serialize, Deserialize};
use bincode;
use std::collections::HashMap;


// Kita akan menggunakan public key sebagai alamat akun
pub type Address = [u8; PUBLIC_KEY_SIZE];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub balance: u64,
    pub nonce: u64, // Untuk mencegah replay attack
}

impl Account {
    pub fn new(balance: u64) -> Self {
        Self { balance, nonce: 0 }
    }
}

// StateMachine akan menjadi 'wrapper' di sekitar database RocksDB kita
pub struct StateMachine {
    db: DB,
}

impl StateMachine {
    /// Membuka atau membuat database baru di path yang diberikan
    pub fn new(path: &str) -> Result<Self, rocksdb::Error> { // <-- Ubah signature fungsi
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?; // <-- Gunakan path dari argumen
        Ok(Self { db })
    }

    /// Mendapatkan state akun dari database berdasarkan alamat
    pub fn get_account(&self, address: &Address) -> Result<Option<Account>, bincode::Error> {
        match self.db.get(address).unwrap() {
            Some(encoded_account) => {
                let account: Account = bincode::deserialize(&encoded_account)?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    /// Menyimpan (atau memperbarui) state akun ke database
    pub fn set_account(&self, address: &Address, account: &Account) -> Result<(), bincode::Error> {
        let encoded_account = bincode::serialize(account)?;
        self.db.put(address, encoded_account).unwrap();
        Ok(())
    }

    // Untuk tujuan testing, kita buat fungsi untuk 'mem-bootstrap' state awal
    pub fn bootstrap_accounts(&self) {
        let genesis_keypair = crate::crypto::KeyPair::new().unwrap();
        let voter_keypair = crate::crypto::KeyPair::new().unwrap();

        // Akun Genesis dengan banyak dana
        let genesis_account = Account::new(1_000_000_000);
        self.set_account(&genesis_keypair.public_key, &genesis_account).unwrap();

        // Akun 'Voter' dengan sedikit dana awal
        let voter_account = Account::new(500);
        self.set_account(&voter_keypair.public_key, &voter_account).unwrap();

        println!("Dumping bootstrap keys (SAVE THESE!):");
        println!("  Genesis Address: 0x{}", hex::encode(genesis_keypair.public_key));
        println!("  Voter Address:   0x{}", hex::encode(voter_keypair.public_key));
        println!("  Voter Private Key (for signing transactions): 0x{}", hex::encode(voter_keypair.private_key));

    }
}