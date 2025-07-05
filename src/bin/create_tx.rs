// src/bin/create_tx.rs

// Impor definisi dari proyek utama kita
use evice_blockchain::blockchain::Transaction;
// --- UBAH: Perbaiki impor ---
use evice_blockchain::crypto::KeyPair;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        eprintln!("Usage: {} <private_key_hex> <recipient_address_hex> <amount> <nonce>", args[0]);
        return;
    }

    let private_key_hex = &args[1];
    let recipient_hex = &args[2];
    let amount: u64 = args[3].parse().expect("Amount must be a number");
    let nonce: u64 = args[4].parse().expect("Nonce must be a number");

    let mut private_key_bytes = [0u8; 32];
    hex::decode_to_slice(private_key_hex, &mut private_key_bytes).expect("Invalid private key hex");

    let keypair = KeyPair::new().unwrap();
    let sender_pub_key = KeyPair::public_key_from_private(&private_key_bytes);

    let mut recipient_bytes = [0u8; 32];
    hex::decode_to_slice(recipient_hex, &mut recipient_bytes).expect("Invalid recipient hex");

    let mut tx = Transaction {
        sender: sender_pub_key,
        recipient: recipient_bytes,
        amount,
        nonce,
        signature: [0u8; 64],
    };

    let message_hash = tx.message_hash();
    let signature = keypair.sign_with_private_key(&message_hash, &private_key_bytes).unwrap();
    
    tx.signature = signature;

    println!("{}", serde_json::to_string_pretty(&tx).unwrap());
}