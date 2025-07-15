// src/mempool.rs

use crate::blockchain::Transaction;
use crate::state::StateMachine;
use std::collections::HashSet;
use std::sync::{ Arc, Mutex };
use log::{ debug, warn };

#[derive(Clone)]
pub struct Mempool {
    transactions: Arc<Mutex<HashSet<Transaction>>>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn add_transaction(
        &self,
        tx: Transaction,
        state: &StateMachine
    ) -> Result<(), &'static str> {
        if !tx.verify() {
            warn!("MEMPOOL: Ditolak, tanda tangan tidak valid.");
            return Err("Tanda tangan tidak valid");
        }

        let sender_account = state
            .get_account(&tx.sender)
            .map_err(|_| "Gagal akses database")?
            .ok_or("Akun pengirim tidak ditemukan")?;

        if tx.nonce < sender_account.nonce {
            warn!(
                "MEMPOOL: Ditolak, nonce sudah usang (expected >= {}, got {}). Kemungkinan replay attack.",
                sender_account.nonce,
                tx.nonce
            );
            return Err("Nonce sudah usang (replay attack?)");
        }

        // --- TAMBAHAN: Validasi saldo di Mempool ---
        if sender_account.balance < tx.amount {
            warn!(
                "MEMPOOL: Ditolak, saldo tidak cukup (memiliki {}, butuh {}).",
                sender_account.balance,
                tx.amount
            );
            return Err("Saldo tidak cukup");
        }

        let mut pool = self.transactions.lock().unwrap();
        if pool.insert(tx) {
            debug!("MEMPOOL: Transaksi baru ditambahkan. Total di mempool: {}", pool.len());
            Ok(())
        } else {
            warn!("MEMPOOL: Ditolak, transaksi sudah ada di mempool.");
            Err("Transaksi sudah ada di mempool")
        }
    }

    pub fn get_transactions(&self, count: usize) -> Vec<Transaction> {
        let mut pool = self.transactions.lock().unwrap();

        let transactions_to_take: Vec<Transaction> = pool.iter().take(count).cloned().collect();

        for tx in &transactions_to_take {
            pool.remove(tx);
        }

        if !transactions_to_take.is_empty() {
            debug!("MEMPOOL: Mengambil {} transaksi untuk blok baru.", transactions_to_take.len());
        }

        transactions_to_take
    }

    pub fn add_from_p2p(&self, tx: Transaction) {
        if tx.verify() {
            let mut pool = self.transactions.lock().unwrap();
            if pool.insert(tx) {
                debug!("MEMPOOL: Transaksi dari P2P ditambahkan. Total di mempool: {}", pool.len());
            }
        } else {
            warn!("MEMPOOL: Transaksi dari P2P ditolak, tanda tangan tidak valid.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::Transaction;
    use crate::crypto::{ KeyPair, SIGNATURE_SIZE };
    use crate::state::{ Account, StateMachine, Address };
    use tempfile::tempdir;

    fn create_test_tx(
        sender_key: &KeyPair,
        recipient: Address,
        amount: u64,
        nonce: u64
    ) -> Transaction {
        let mut tx = Transaction {
            sender: sender_key.public_key_bytes(), // <-- PERBAIKAN
            recipient,
            amount,
            nonce,
            signature: [0; SIGNATURE_SIZE],
        };
        let hash = tx.message_hash();
        tx.signature = sender_key.sign(&hash); // <-- PERBAIKAN
        tx
    }

    #[test]
    fn test_add_valid_transaction() {
        let dir = tempdir().unwrap();
        let state = StateMachine::new(dir.path().to_str().unwrap()).unwrap();
        let mempool = Mempool::new();
        let user1_keys = KeyPair::new(); // <-- PERBAIKAN
        let user2_address: Address = KeyPair::new().public_key_bytes(); // <-- PERBAIKAN
        let user1_account = Account { balance: 1000, nonce: 0 };
        state.set_account(&user1_keys.public_key_bytes(), &user1_account).unwrap();
        let tx = create_test_tx(&user1_keys, user2_address, 100, 0);

        let result = mempool.add_transaction(tx.clone(), &state);
        assert!(result.is_ok());
        assert_eq!(mempool.transactions.lock().unwrap().len(), 1);
        assert!(mempool.transactions.lock().unwrap().contains(&tx));
    }

    #[test]
    fn test_reject_stale_nonce() {
        let dir = tempdir().unwrap();
        let state = StateMachine::new(dir.path().to_str().unwrap()).unwrap();
        let mempool = Mempool::new();
        let user1_keys = KeyPair::new(); // <-- PERBAIKAN
        let user2_address: Address = KeyPair::new().public_key_bytes(); // <-- PERBAIKAN
        let user1_account = Account { balance: 1000, nonce: 5 };
        state.set_account(&user1_keys.public_key_bytes(), &user1_account).unwrap();
        let tx = create_test_tx(&user1_keys, user2_address, 100, 0);

        let result = mempool.add_transaction(tx, &state);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Nonce sudah usang (replay attack?)");
        assert_eq!(mempool.transactions.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_reject_insufficient_balance() {
        let dir = tempdir().unwrap();
        let state = StateMachine::new(dir.path().to_str().unwrap()).unwrap();
        let mempool = Mempool::new();
        let user1_keys = KeyPair::new(); // <-- PERBAIKAN
        let user2_address: Address = KeyPair::new().public_key_bytes(); // <-- PERBAIKAN
        let user1_account = Account { balance: 50, nonce: 0 };
        state.set_account(&user1_keys.public_key_bytes(), &user1_account).unwrap();
        let tx = create_test_tx(&user1_keys, user2_address, 100, 0);

        let result = mempool.add_transaction(tx, &state);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Saldo tidak cukup");
        assert_eq!(mempool.transactions.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_reject_duplicate_transaction() {
        let dir = tempdir().unwrap();
        let state = StateMachine::new(dir.path().to_str().unwrap()).unwrap();
        let mempool = Mempool::new();
        let user1_keys = KeyPair::new(); // <-- PERBAIKAN
        let user2_address: Address = KeyPair::new().public_key_bytes(); // <-- PERBAIKAN
        let user1_account = Account { balance: 1000, nonce: 0 };
        state.set_account(&user1_keys.public_key_bytes(), &user1_account).unwrap();
        let tx = create_test_tx(&user1_keys, user2_address, 100, 0);

        let result1 = mempool.add_transaction(tx.clone(), &state);
        assert!(result1.is_ok());
        assert_eq!(mempool.transactions.lock().unwrap().len(), 1);

        let result2 = mempool.add_transaction(tx, &state);
        assert!(result2.is_err());
        assert_eq!(result2.unwrap_err(), "Transaksi sudah ada di mempool");
        assert_eq!(mempool.transactions.lock().unwrap().len(), 1);
    }
}