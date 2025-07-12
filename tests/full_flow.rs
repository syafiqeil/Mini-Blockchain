// tests/full_flow.rs

use evice_blockchain::{
    blockchain::{Blockchain, Transaction},
    crypto::KeyPair,
    mempool::Mempool,
    state::{Account, Address},
};
use tempfile::tempdir;

#[test]
fn test_full_transaction_to_block_flow() {
    // 1. SETUP: Siapkan semua komponen yang dibutuhkan
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().to_str().unwrap();

    let authority_keys = KeyPair::new().unwrap();
    let user_keys = KeyPair::new().unwrap();
    let recipient_address: Address = KeyPair::new().unwrap().public_key;

    let mut blockchain = Blockchain::new(db_path);
    let mempool = Mempool::new();

    // Beri user saldo awal
    let initial_user_account = Account { balance: 1000, nonce: 0 };
    blockchain.state.set_account(&user_keys.public_key, &initial_user_account).unwrap();
    
    // 2. EKSEKUSI: Lakukan alur kerja transaksi
    // Buat transaksi
    let mut tx = Transaction {
        sender: user_keys.public_key,
        recipient: recipient_address,
        amount: 150,
        nonce: 0,
        signature: [0; 64],
    };
    let hash = tx.message_hash();
    tx.signature = user_keys.sign(&hash).unwrap();

    // Tambahkan ke mempool
    assert!(mempool.add_transaction(tx, &blockchain.state).is_ok());

    // Otoritas mengambil transaksi dari mempool
    let transactions_for_block = mempool.get_transactions(1);
    assert_eq!(transactions_for_block.len(), 1);

    // Otoritas membuat dan memproses blok baru
    let new_block = blockchain.create_block(&authority_keys, transactions_for_block);
    let result = blockchain.add_block(new_block);
    assert!(result, "Penambahan blok seharusnya berhasil");

    // 3. ASERSI: Verifikasi keadaan akhir (final state)
    // Cek ketinggian blockchain
    assert_eq!(blockchain.chain.len(), 2, "Blockchain seharusnya memiliki 2 blok (genesis + 1)");

    // Cek akun pengirim
    let final_user_account = blockchain.state.get_account(&user_keys.public_key).unwrap().unwrap();
    assert_eq!(final_user_account.balance, 850, "Saldo pengirim seharusnya berkurang"); // 1000 - 150
    assert_eq!(final_user_account.nonce, 1, "Nonce pengirim seharusnya bertambah");
    
    // Cek akun penerima
    let final_recipient_account = blockchain.state.get_account(&recipient_address).unwrap().unwrap();
    assert_eq!(final_recipient_account.balance, 150, "Saldo penerima seharusnya bertambah");

    // Cek mempool
    assert_eq!(mempool.get_transactions(1).len(), 0, "Mempool seharusnya kosong setelah transaksi diproses");
}