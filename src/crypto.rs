// src/crypto.rs

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(dead_code)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

// Definisikan konstanta kita di Rust juga
pub const PUBLIC_KEY_SIZE: usize = ffi::PUBLIC_KEY_SIZE as usize;
pub const PRIVATE_KEY_SIZE: usize = ffi::PRIVATE_KEY_SIZE as usize;
pub const SIGNATURE_SIZE: usize = ffi::SIGNATURE_SIZE as usize;

// Ini adalah wrapper aman kita untuk pointer C++
pub struct KeyPair {
    // Pointer mentah ke struct C++
    ptr: *mut ffi::Ed25519KeyPair,
    pub public_key: [u8; PUBLIC_KEY_SIZE],
    pub private_key: [u8; PRIVATE_KEY_SIZE],
}

impl KeyPair {
    /// Membuat keypair baru dengan memanggil fungsi C++.
    pub fn new() -> Option<Self> {
        let pair_ptr = unsafe { ffi::create_keypair() };
        if pair_ptr.is_null() {
            return None;
        }

        let mut public_key = [0u8; PUBLIC_KEY_SIZE];
        let mut private_key = [0u8; PRIVATE_KEY_SIZE];

        unsafe {
            ffi::get_keys_from_pair(pair_ptr, public_key.as_mut_ptr(), private_key.as_mut_ptr());
        }

        Some(Self {
            ptr: pair_ptr,
            public_key,
            private_key,
        })
    }
    
    /// Menandatangani pesan. Mengembalikan sebuah signature.
    pub fn sign(&self, message: &[u8]) -> Option<[u8; SIGNATURE_SIZE]> {
        let mut signature = [0u8; SIGNATURE_SIZE];
        let result = unsafe {
            ffi::sign_message(
                self.ptr,
                message.as_ptr(),
                message.len(),
                signature.as_mut_ptr(),
            )
        };

        if result == 1 {
            Some(signature)
        } else {
            None
        }
    }
}

/// Fungsi verifikasi global yang aman
pub fn verify(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    if public_key.len() != PUBLIC_KEY_SIZE || signature.len() != SIGNATURE_SIZE {
        return false;
    }
    
    let result = unsafe {
        ffi::verify_signature(
            public_key.as_ptr(),
            message.as_ptr(),
            message.len(),
            signature.as_ptr(),
        )
    };

    result == 1
}


// Implementasi trait Drop (RAII).
// Metode ini akan otomatis dipanggil ketika sebuah instance `KeyPair`
// keluar dari scope, memastikan tidak ada memory leak.
impl Drop for KeyPair {
    fn drop(&mut self) {
        // Panggil fungsi C++ untuk membebaskan memori.
        unsafe {
            ffi::free_keypair(self.ptr);
        }
    }
}

unsafe impl Send for KeyPair {}
unsafe impl Sync for KeyPair {}