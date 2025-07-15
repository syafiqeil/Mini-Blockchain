// src/bin/create_tx.rs

use evice_blockchain::blockchain::Transaction;
use evice_blockchain::crypto::{self, PUBLIC_KEY_SIZE, PRIVATE_KEY_SIZE, SIGNATURE_SIZE};
use pqcrypto_traits::sign::{SecretKey as _, DetachedSignature as _};
use pqcrypto_dilithium::dilithium2::{detached_sign, SecretKey};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 6 {
        eprintln!("Usage: {} <private_key_hex> <SENDER_public_key_hex> <recipient_address_hex> <amount> <nonce>", args[0]);
        return;
    }

    let private_key_hex = &args[1];
    let public_key_hex = &args[2];
    let recipient_hex = &args[3];
    let amount: u64 = args[4].parse().expect("Amount must be a number");
    let nonce: u64 = args[5].parse().expect("Nonce must be a number");

    // Muat private key dari argumen untuk menandatangani
    let mut private_key_bytes = [0u8; PRIVATE_KEY_SIZE]; 
    hex::decode_to_slice(private_key_hex, &mut private_key_bytes).expect("Invalid private key hex");
    let sk = SecretKey::from_bytes(&private_key_bytes).expect("Failed to create secret key from bytes");

    // Muat public key (sender) dari argumen untuk data transaksi
    let mut sender_pub_key_bytes = [0u8; PUBLIC_KEY_SIZE];
    hex::decode_to_slice(public_key_hex, &mut sender_pub_key_bytes).expect("Invalid sender public key hex");
    
    // Muat public key (recipient) dari argumen
    let mut recipient_bytes = [0u8; PUBLIC_KEY_SIZE];
    hex::decode_to_slice(recipient_hex, &mut recipient_bytes).expect("Invalid recipient hex");

    let mut tx = Transaction {
        sender: sender_pub_key_bytes,
        recipient: recipient_bytes,
        amount,
        nonce,
        signature: [0u8; SIGNATURE_SIZE],
    };

    let message_hash = tx.message_hash();
    // Gunakan fungsi sign langsung dari pustaka dengan secret key yang sudah dimuat
    let signature_struct = detached_sign(&message_hash, &sk);
    
    tx.signature = signature_struct.as_bytes().try_into().unwrap();

    let json_output = serde_json::to_string_pretty(&tx).expect("Gagal membuat JSON transaksi");
    println!("{}", json_output);
}