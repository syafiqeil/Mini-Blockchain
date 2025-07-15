// src/crypto.rs

use pqcrypto_traits::sign::{
    PublicKey as PublicKeyTrait, 
    SecretKey as SecretKeyTrait,
    DetachedSignature as DetachedSignatureTrait
};
use pqcrypto_dilithium::dilithium2::{
    keypair, detached_sign, verify_detached_signature,
    PublicKey, SecretKey, DetachedSignature,
};

pub const PUBLIC_KEY_SIZE: usize = 1312;
pub const PRIVATE_KEY_SIZE: usize = 2560;
pub const SIGNATURE_SIZE: usize = 2420;

pub struct KeyPair {
    pub public_key: PublicKey,
    pub private_key: SecretKey,
}

impl KeyPair {
    pub fn new() -> Self {
        let (pk, sk) = keypair();
        Self { public_key: pk, private_key: sk }
    }

    pub fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_SIZE] {
        let signature = detached_sign(message, &self.private_key);
        signature.as_bytes().try_into().expect("Signature length mismatch")
    }

    pub fn public_key_bytes(&self) -> [u8; PUBLIC_KEY_SIZE] {
        self.public_key.as_bytes().try_into().expect("Public key length mismatch")
    }

    pub fn private_key_bytes(&self) -> [u8; PRIVATE_KEY_SIZE] {
        self.private_key.as_bytes().try_into().expect("Secret key length mismatch")
    }
}

pub fn verify(public_key_bytes: &[u8], message: &[u8], signature_bytes: &[u8]) -> bool {
    let pk = match PublicKey::from_bytes(public_key_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    let sig = match DetachedSignature::from_bytes(signature_bytes) {
        Ok(sig) => sig,
        Err(_) => return false,
    };
    
    verify_detached_signature(&sig, message, &pk).is_ok()
}