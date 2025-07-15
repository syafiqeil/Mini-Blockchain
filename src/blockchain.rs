// src/blockchain.rs

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{info, warn, error};

use crate::crypto::{self, KeyPair, PUBLIC_KEY_SIZE, SIGNATURE_SIZE};
use crate::state::{Account, Address, StateMachine};

pub type PublicKey = [u8; PUBLIC_KEY_SIZE];
pub type Signature = [u8; SIGNATURE_SIZE];

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Transaction {
    #[serde(with = "serde_bytes")]
    pub sender: Address,
    #[serde(with = "serde_bytes")]
    pub recipient: Address,
    pub amount: u64,
    pub nonce: u64,
    #[serde(with = "serde_bytes")]
    pub signature: Signature,
}

impl Transaction {
    pub fn message_hash(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.sender);
        data.extend_from_slice(&self.recipient);
        data.extend_from_slice(&self.amount.to_be_bytes());
        data.extend_from_slice(&self.nonce.to_be_bytes());

        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    pub fn verify(&self) -> bool {
        let hash = self.message_hash();
        crypto::verify(&self.sender, &hash, &self.signature)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub index: u64,
    pub timestamp: u128,
    pub prev_hash: Vec<u8>,
    pub hash: Vec<u8>,
    pub transactions: Vec<Transaction>,
    #[serde(with = "serde_bytes")]
    pub signature: Signature,
    #[serde(with = "serde_bytes")]
    pub authority: PublicKey,
}

// --- PERBAIKAN: Mengembalikan enum ChainMessage yang hilang ---
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChainMessage {
    NewBlock(Block),
    NewTransaction(Transaction),
}

impl Block {
    pub fn genesis() -> Self {
        let mut block = Block {
            index: 0,
            timestamp: 1704067200000,
            prev_hash: vec![0; 32],
            hash: Vec::new(),
            transactions: vec![],
            signature: [0; SIGNATURE_SIZE],
            authority: [0; PUBLIC_KEY_SIZE],
        };
        block.hash = Self::calculate_hash(&block);
        block
    }

    pub fn calculate_hash(block: &Block) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&block.index.to_be_bytes());
        data.extend_from_slice(&block.timestamp.to_be_bytes());
        data.extend_from_slice(&block.prev_hash);
        data.extend_from_slice(&block.authority);
        let mut tx_hashes = Vec::new();
        for tx in &block.transactions {
            tx_hashes.extend_from_slice(&tx.message_hash());
        }
        let mut merkle_hasher = Sha256::new();
        merkle_hasher.update(tx_hashes);
        data.extend_from_slice(&merkle_hasher.finalize());

        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

pub struct Blockchain {
    pub chain: Vec<Block>,
    pub state: StateMachine,
}

impl Blockchain {
    pub fn new(db_path: &str) -> Self {
        let state = StateMachine::new(db_path).expect("Gagal membuka database state");
        Self {
            chain: vec![Block::genesis()],
            state,
        }
    }

    pub fn create_block(&self, authority_keypair: &KeyPair, transactions: Vec<Transaction>) -> Block {
        let last_block = self.chain.last().expect("Chain tidak boleh kosong");
        let new_index = last_block.index + 1;
        let new_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let mut new_block = Block {
            index: new_index,
            timestamp: new_timestamp,
            prev_hash: last_block.hash.clone(),
            hash: Vec::new(),
            transactions,
            signature: [0; SIGNATURE_SIZE],
            authority: authority_keypair.public_key_bytes(),
        };

        let hash = Block::calculate_hash(&new_block);
        new_block.hash = hash.clone();
        new_block.signature = authority_keypair.sign(&hash);

        new_block
    }

    pub fn add_block(&mut self, block: Block) -> bool {
        let last_block = self.chain.last().unwrap();

        if block.index != last_block.index + 1 {
            warn!("Validasi Gagal: Index tidak valid (expected {}, got {})", last_block.index + 1, block.index);
            return false;
        }
        if block.prev_hash != last_block.hash {
            warn!("Validasi Gagal: Previous hash tidak cocok");
            return false;
        }
        let calculated_hash = Block::calculate_hash(&block);
        if block.hash != calculated_hash {
            warn!("Validasi Gagal: Hash blok tidak valid");
            return false;
        }
        if !crypto::verify(&block.authority, &block.hash, &block.signature) {
            warn!("Validasi Gagal: Tanda tangan authority tidak valid");
            return false;
        }

        let mut temp_block_state: HashMap<Address, Account> = HashMap::new();

        for tx in &block.transactions {
            if !tx.verify() {
                warn!("Validasi Gagal: Tanda tangan transaksi tidak valid dalam blok {}", block.index);
                return false;
            }
            if let Err(e) = self.state.validate_transaction_in_block(tx, &mut temp_block_state) {
                warn!("Validasi Gagal: Transaksi tidak valid dalam blok {}. Alasan: {}", block.index, e);
                return false;
            }
        }

        let mut batch = rocksdb::WriteBatch::default();
        for (address, account) in temp_block_state {
            let encoded_account = bincode::serialize(&account).unwrap();
            batch.put(address, encoded_account);
        }

        if let Err(e) = self.state.db.write(batch) {
            error!("KRITIS: Gagal menulis batch state ke database: {}", e);
            return false;
        }

        info!(
            "Blok baru #{} divalidasi dan ditambahkan ke chain dengan {} transaksi.",
            block.index,
            block.transactions.len()
        );
        self.chain.push(block);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // Helper function to create a signed transaction for tests
    fn create_test_tx(sender_key: &KeyPair, recipient: Address, amount: u64, nonce: u64) -> Transaction {
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
    fn test_add_valid_block() {
        // Setup
        let dir = tempdir().unwrap();
        let mut blockchain = Blockchain::new(dir.path().to_str().unwrap());
        let authority = KeyPair::new(); // <-- PERBAIKAN
        let user1 = KeyPair::new(); // <-- PERBAIKAN
        let user2_address = KeyPair::new().public_key_bytes(); // <-- PERBAIKAN

        // Setup initial state
        let user1_account = Account { balance: 1000, nonce: 0 };
        blockchain.state.set_account(&user1.public_key_bytes(), &user1_account).unwrap();
        
        let tx = create_test_tx(&user1, user2_address, 100, 0);
        let block = blockchain.create_block(&authority, vec![tx]);

        // Action
        let result = blockchain.add_block(block);

        // Assertions
        assert!(result);
        assert_eq!(blockchain.chain.len(), 2);
        let updated_user1_account = blockchain.state.get_account(&user1.public_key_bytes()).unwrap().unwrap();
        assert_eq!(updated_user1_account.balance, 900);
        assert_eq!(updated_user1_account.nonce, 1);
    }

    #[test]
    fn test_reject_block_with_bad_prev_hash() {
        let dir = tempdir().unwrap();
        let mut blockchain = Blockchain::new(dir.path().to_str().unwrap());
        let authority = KeyPair::new(); // <-- PERBAIKAN
        
        let mut block = blockchain.create_block(&authority, vec![]);
        block.prev_hash = vec![1, 2, 3];

        let result = blockchain.add_block(block);
        assert!(!result);
        assert_eq!(blockchain.chain.len(), 1);
    }

    #[test]
    fn test_reject_block_with_bad_signature() {
        let dir = tempdir().unwrap();
        let mut blockchain = Blockchain::new(dir.path().to_str().unwrap());
        let authority = KeyPair::new(); // <-- PERBAIKAN
        let fake_authority = KeyPair::new(); // <-- PERBAIKAN
        
        let mut block = blockchain.create_block(&authority, vec![]);
        let hash = Block::calculate_hash(&block);
        block.signature = fake_authority.sign(&hash); 

        let result = blockchain.add_block(block);
        assert!(!result);
        assert_eq!(blockchain.chain.len(), 1);
    }

    #[test]
    fn test_atomic_revert_on_invalid_transaction() {
        let dir = tempdir().unwrap();
        let mut blockchain = Blockchain::new(dir.path().to_str().unwrap());
        let authority = KeyPair::new(); // <-- PERBAIKAN
        let user1 = KeyPair::new(); // <-- PERBAIKAN
        let user2 = KeyPair::new(); // <-- PERBAIKAN
        let user3_address = KeyPair::new().public_key_bytes(); // <-- PERBAIKAN

        let user1_account = Account { balance: 1000, nonce: 0 };
        blockchain.state.set_account(&user1.public_key_bytes(), &user1_account).unwrap();
        let user2_account = Account { balance: 50, nonce: 0 };
        blockchain.state.set_account(&user2.public_key_bytes(), &user2_account).unwrap();
        
        let valid_tx = create_test_tx(&user1, user3_address, 100, 0);
        let invalid_tx = create_test_tx(&user2, user3_address, 100, 0);
        
        let block = blockchain.create_block(&authority, vec![valid_tx, invalid_tx]);

        let result = blockchain.add_block(block);
        assert!(!result);
        assert_eq!(blockchain.chain.len(), 1);

        let user1_account_after = blockchain.state.get_account(&user1.public_key_bytes()).unwrap().unwrap();
        assert_eq!(user1_account_after.balance, 1000);
        assert_eq!(user1_account_after.nonce, 0);
    }
}
