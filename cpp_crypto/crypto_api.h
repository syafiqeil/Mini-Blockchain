#ifndef CRYPTO_API_H
#define CRYPTO_API_H

#include <stddef.h>
#include <stdint.h>

// Ukuran dalam byte untuk kunci dan tanda tangan Ed25519
#define PUBLIC_KEY_SIZE 32
#define PRIVATE_KEY_SIZE 32
#define SIGNATURE_SIZE 64

#ifdef __cplusplus
extern "C" {
#endif

// Struktur untuk menampung keypair yang dialokasikan di C++
typedef struct Ed25519KeyPair Ed25519KeyPair;

// Menghasilkan keypair baru dan mengembalikannya sebagai pointer.
// Rust akan bertanggung jawab untuk memanggil free_keypair nanti.
Ed25519KeyPair* create_keypair();

// Membebaskan memori yang dialokasikan untuk keypair.
void free_keypair(Ed25519KeyPair* pair);

// Menyalin kunci publik dan privat dari struct ke buffer yang disediakan Rust.
void get_keys_from_pair(const Ed25519KeyPair* pair, uint8_t* public_key_out, uint8_t* private_key_out);

// Menandatangani sebuah pesan (hash) menggunakan private key dari keypair.
// Mengembalikan 0 jika gagal, 1 jika berhasil.
int sign_message(const Ed25519KeyPair* pair, const uint8_t* message, size_t message_len, uint8_t* signature_out);

// Memverifikasi tanda tangan terhadap sebuah pesan dan public key.
// Mengembalikan 1 jika valid, 0 jika tidak valid, -1 jika error.
int verify_signature(const uint8_t* public_key, const uint8_t* message, size_t message_len, const uint8_t* signature);

#ifdef __cplusplus
}
#endif

#endif // CRYPTO_API_H