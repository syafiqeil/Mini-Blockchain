// tests/full_flow.rs

use evice_blockchain::{
    blockchain::{Blockchain, Transaction},
    crypto::{KeyPair, SIGNATURE_SIZE}, 
    mempool::Mempool,
    state::{Account, Address},
};
use tempfile::tempdir;

#[test]
fn test_full_transaction_to_block_flow() {
    let db_dir = tempdir().unwrap();
    let db_path = db_dir.path().to_str().unwrap();

    let authority_keys = KeyPair::new(); 
    let user_keys = KeyPair::new(); 
    let recipient_address: Address = KeyPair::new().public_key_bytes(); 

    let mut blockchain = Blockchain::new(db_path);
    let mempool = Mempool::new();

    let initial_user_account = Account { balance: 1000, nonce: 0 };
    blockchain.state.set_account(&user_keys.public_key_bytes(), &initial_user_account).unwrap();
    
    let mut tx = Transaction {
        sender: user_keys.public_key_bytes(), 
        recipient: recipient_address,
        amount: 150,
        nonce: 0,
        signature: [0; SIGNATURE_SIZE], 
    };
    let hash = tx.message_hash();
    tx.signature = user_keys.sign(&hash); 

    assert!(mempool.add_transaction(tx, &blockchain.state).is_ok());

    let transactions_for_block = mempool.get_transactions(1);
    assert_eq!(transactions_for_block.len(), 1);

    let new_block = blockchain.create_block(&authority_keys, transactions_for_block);
    let result = blockchain.add_block(new_block);
    assert!(result, "Penambahan blok seharusnya berhasil");

    assert_eq!(blockchain.chain.len(), 2, "Blockchain seharusnya memiliki 2 blok (genesis + 1)");

    let final_user_account = blockchain.state.get_account(&user_keys.public_key_bytes()).unwrap().unwrap();
    assert_eq!(final_user_account.balance, 850, "Saldo pengirim seharusnya berkurang");
    assert_eq!(final_user_account.nonce, 1, "Nonce pengirim seharusnya bertambah");
    
    let final_recipient_account = blockchain.state.get_account(&recipient_address).unwrap().unwrap();
    assert_eq!(final_recipient_account.balance, 150, "Saldo penerima seharusnya bertambah");

    assert_eq!(mempool.get_transactions(1).len(), 0, "Mempool seharusnya kosong setelah transaksi diproses");
}