// src/state.rs

use rocksdb::{DB, Options};
use crate::crypto::{PUBLIC_KEY_SIZE};
use serde::{Serialize, Deserialize};
use bincode;
// --- UBAH: Hapus 'use std::collections::HashMap;' ---
use crate::blockchain::Transaction;

// ... sisa file state.rs tetap sama ...

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

pub struct StateMachine {
    db: DB,
}

impl StateMachine {
    pub fn new(path: &str) -> Result<Self, rocksdb::Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    pub fn get_account(&self, address: &Address) -> Result<Option<Account>, bincode::Error> {
        match self.db.get(address).unwrap() {
            Some(encoded_account) => {
                let account: Account = bincode::deserialize(&encoded_account)?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    pub fn set_account(&self, address: &Address, account: &Account) -> Result<(), bincode::Error> {
        let encoded_account = bincode::serialize(account)?;
        self.db.put(address, encoded_account).unwrap();
        Ok(())
    }

    pub fn process_transaction(&mut self, tx: &Transaction) -> bool {
        if !tx.verify() {
            eprintln!("STATE: Tanda tangan transaksi tidak valid");
            return false;
        }

        let mut sender_account = match self.get_account(&tx.sender).unwrap() {
            Some(acc) => acc,
            None => {
                eprintln!("STATE: Akun pengirim tidak ditemukan");
                return false;
            }
        };

        let mut recipient_account = self.get_account(&tx.recipient).unwrap().unwrap_or_else(|| Account::new(0));

        if tx.nonce != sender_account.nonce {
             eprintln!("STATE: Nonce tidak valid. Expected: {}, Got: {}", sender_account.nonce, tx.nonce);
            return false;
        }

        if sender_account.balance < tx.amount {
            eprintln!("STATE: Saldo tidak cukup");
            return false;
        }

        sender_account.balance -= tx.amount;
        recipient_account.balance += tx.amount;
        sender_account.nonce += 1;

        self.set_account(&tx.sender, &sender_account).unwrap();
        self.set_account(&tx.recipient, &recipient_account).unwrap();

        true
    }

    pub fn bootstrap_accounts(&self) {
        let genesis_keypair = crate::crypto::KeyPair::new().unwrap();
        let voter_keypair = crate::crypto::KeyPair::new().unwrap();

        let genesis_account = Account::new(1_000_000_000);
        self.set_account(&genesis_keypair.public_key, &genesis_account).unwrap();

        let voter_account = Account::new(500);
        self.set_account(&voter_keypair.public_key, &voter_account).unwrap();

        println!("Dumping bootstrap keys (SAVE THESE!):");
        println!("  Genesis Address: 0x{}", hex::encode(genesis_keypair.public_key));
        println!("  Voter Address:   0x{}", hex::encode(voter_keypair.public_key));
        println!("  Voter Private Key (for signing transactions): 0x{}", hex::encode(voter_keypair.private_key));
    }
}