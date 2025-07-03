#include "crypto_api.h"
#include <openssl/evp.h>
#include <openssl/err.h>
#include <vector>

struct Ed25519KeyPair {
    EVP_PKEY* pkey;
};

Ed25519KeyPair* create_keypair() {
    EVP_PKEY* pkey = NULL;
    EVP_PKEY_CTX* pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_ED25519, NULL);
    if (!pctx) return NULL;
    if (EVP_PKEY_keygen_init(pctx) <= 0) {
        EVP_PKEY_CTX_free(pctx);
        return NULL;
    }
    if (EVP_PKEY_keygen(pctx, &pkey) <= 0) {
        EVP_PKEY_CTX_free(pctx);
        return NULL;
    }
    EVP_PKEY_CTX_free(pctx);
    
    Ed25519KeyPair* pair = new Ed25519KeyPair;
    pair->pkey = pkey;
    return pair;
}

void free_keypair(Ed25519KeyPair* pair) {
    if (pair) {
        EVP_PKEY_free(pair->pkey);
        delete pair;
    }
}

void get_keys_from_pair(const Ed25519KeyPair* pair, uint8_t* public_key_out, uint8_t* private_key_out) {
    if (!pair || !pair->pkey) return;
    size_t pub_len = PUBLIC_KEY_SIZE;
    size_t priv_len = PRIVATE_KEY_SIZE;
    EVP_PKEY_get_raw_public_key(pair->pkey, public_key_out, &pub_len);
    EVP_PKEY_get_raw_private_key(pair->pkey, private_key_out, &priv_len);
}

int sign_message(const Ed25519KeyPair* pair, const uint8_t* message, size_t message_len, uint8_t* signature_out) {
    if (!pair || !pair->pkey || !message || !signature_out) return 0;

    EVP_MD_CTX* mdctx = EVP_MD_CTX_new();
    if (!mdctx) return 0;

    size_t sig_len = SIGNATURE_SIZE;
    
    if (EVP_DigestSignInit(mdctx, NULL, NULL, NULL, pair->pkey) <= 0) {
        EVP_MD_CTX_free(mdctx);
        return 0;
    }

    if (EVP_DigestSign(mdctx, signature_out, &sig_len, message, message_len) <= 0) {
        EVP_MD_CTX_free(mdctx);
        return 0;
    }

    EVP_MD_CTX_free(mdctx);
    return 1;
}

int verify_signature(const uint8_t* public_key, const uint8_t* message, size_t message_len, const uint8_t* signature) {
    if (!public_key || !message || !signature) return -1;
    
    EVP_PKEY* pkey = EVP_PKEY_new_raw_public_key(EVP_PKEY_ED25519, NULL, public_key, PUBLIC_KEY_SIZE);
    if (!pkey) return -1;

    EVP_MD_CTX* mdctx = EVP_MD_CTX_new();
    if (!mdctx) {
        EVP_PKEY_free(pkey);
        return -1;
    }

    if (EVP_DigestVerifyInit(mdctx, NULL, NULL, NULL, pkey) <= 0) {
        EVP_MD_CTX_free(mdctx);
        EVP_PKEY_free(pkey);
        return -1;
    }

    int result = EVP_DigestVerify(mdctx, signature, SIGNATURE_SIZE, message, message_len);
    
    EVP_MD_CTX_free(mdctx);
    EVP_PKEY_free(pkey);

    return result; // 1 = success, 0 = fail, <0 = error
}