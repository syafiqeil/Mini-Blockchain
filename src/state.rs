// src/state.rs

use bincode;
use rocksdb::{Options, DB};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::blockchain::TransactionData;
use crate::blockchain::Transaction;
use crate::crypto::PUBLIC_KEY_SIZE;

// --- TAMBAHAN: Impor makro log ---
use log::{info, warn};

pub type Address = [u8; PUBLIC_KEY_SIZE];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Account {
    pub balance: u64,
    pub staked_amount: u64,
    pub nonce: u64,
}

impl Account {
    pub fn new(balance: u64) -> Self {
        Self { balance, staked_amount: 0, nonce: 0 }
    }
}

pub struct StateMachine {
    pub db: DB,
}

impl StateMachine {
    pub fn new(path: &str) -> Result<Self, rocksdb::Error> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    pub fn get_account(&self, address: &Address) -> Result<Option<Account>, bincode::Error> {
        match self.db.get(address) {
            Ok(Some(encoded_account)) => {
                let account: Account = bincode::deserialize(&encoded_account)?;
                Ok(Some(account))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(bincode::Error::new(bincode::ErrorKind::Custom(
                e.to_string(),
            ))),
        }
    }

    pub fn set_account(&self, address: &Address, account: &Account) -> Result<(), bincode::Error> {
        let encoded_account = bincode::serialize(account)?;
        self.db
            .put(address, encoded_account)
            .map_err(|e| bincode::Error::new(bincode::ErrorKind::Custom(e.to_string())))?;
        Ok(())
    }

    pub fn validate_transaction_in_block(
        &self,
        tx: &Transaction,
        temp_block_state: &mut HashMap<Address, Account>,
    ) -> Result<(), String> {
        let mut sender_account = if let Some(acc) = temp_block_state.get(&tx.sender) {
            acc.clone()
        } else {
            self.get_account(&tx.sender)
                .map_err(|e| format!("STATE: Gagal membaca database akun pengirim: {}", e))?
                .ok_or_else(|| "STATE: Akun pengirim tidak ditemukan di database".to_string())?
        };

        if tx.nonce != sender_account.nonce {
            warn!("STATE: Nonce tidak valid (expected {}, got {}).", sender_account.nonce, tx.nonce);
            return Err("STATE: Nonce tidak valid".to_string());
        }

        sender_account.nonce += 1;

        // Proses data transaksi sesuai jenisnya
        match &tx.data {
            TransactionData::Transfer { recipient, amount } => {
                if sender_account.balance < *amount + tx.fee {
                    return Err(format!("STATE: Saldo tidak cukup untuk transfer + biaya (butuh {}, punya {})", amount + tx.fee, sender_account.balance));
                }
                sender_account.balance -= *amount + tx.fee;
                
                let mut recipient_account = if let Some(acc) = temp_block_state.get(recipient) {
                    acc.clone()
                } else {
                    self.get_account(recipient)
                        .map_err(|e| format!("STATE: Gagal membaca database akun penerima: {}", e))?
                        .unwrap_or_else(|| Account::new(0))
                };

                recipient_account.balance += *amount;
                temp_block_state.insert(*recipient, recipient_account);
            }
            TransactionData::Stake { amount } => {
                if sender_account.balance < *amount + tx.fee {
                    return Err(format!("STATE: Saldo tidak cukup untuk stake + biaya (butuh {}, punya {})", amount + tx.fee, sender_account.balance));
                }
                sender_account.balance -= *amount + tx.fee;
                sender_account.staked_amount += *amount;
            }
        }

        temp_block_state.insert(tx.sender, sender_account);
        Ok(())
    }

    pub fn bootstrap_accounts(&self) {
        let genesis_keypair = crate::crypto::KeyPair::new();
        let voter_keypair = crate::crypto::KeyPair::new();

        let genesis_account = Account::new(1_000_000_000);
        self.set_account(&genesis_keypair.public_key_bytes(), &genesis_account)
            .unwrap();

        let voter_account = Account::new(500);
        self.set_account(&voter_keypair.public_key_bytes(), &voter_account)
            .unwrap();

        info!("Dumping bootstrap keys (SAVE THESE!):");
        info!("  Genesis Address: 0x{}", hex::encode(genesis_keypair.public_key_bytes()));
        info!("  Voter Address:   0x{}", hex::encode(voter_keypair.public_key_bytes()));
        info!("  Voter Private Key (for signing transactions): 0x{}", hex::encode(voter_keypair.private_key_bytes()));
    }
}