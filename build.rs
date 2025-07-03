// C:\AllProject\evice_blockchain\build.rs

use std::env;
use std::path::PathBuf;

fn main() {
    // --- Langkah 1: Dapatkan Path OpenSSL dari Environment Variable ---
    let openssl_dir_str = env::var("OPENSSL_DIR")
        .expect("ERROR: Environment variable OPENSSL_DIR not set. Please set it to your vcpkg OpenSSL installation path.");

    let openssl_dir = PathBuf::from(openssl_dir_str);
    let openssl_include_path = openssl_dir.join("include");
    let openssl_lib_path = openssl_dir.join("lib");

    // --- Langkah 2: Kompilasi Pustaka C++ ---
    // Beritahu 'cc' di mana menemukan header OpenSSL
    cc::Build::new()
        .cpp(true)
        .file("cpp_crypto/crypto_api.cpp")
        .include(&openssl_include_path) // <-- Tambahan PENTING
        .compile("crypto_api");

    // --- Langkah 3: Hasilkan Bindings Rust ---
    // Beritahu 'bindgen' di mana menemukan header OpenSSL
    let bindings = bindgen::Builder::default()
        .header("cpp_crypto/crypto_api.h")
        // Tambahkan argumen ke Clang (digunakan oleh bindgen)
        .clang_arg(format!("-I{}", openssl_include_path.display())) // <-- Tambahan PENTING
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Tidak dapat menghasilkan bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Tidak dapat menulis bindings!");

    // --- Langkah 4: Atur Linker ---
    // Beritahu Cargo di mana menemukan library OpenSSL (.lib)
    println!("cargo:rustc-link-search=native={}", openssl_lib_path.display());
    println!("cargo:rustc-link-lib=static=libssl");
    println!("cargo:rustc-link-lib=static=libcrypto");
    // Diperlukan oleh OpenSSL di Windows
    println!("cargo:rustc-link-lib=gdi32");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=crypt32");
    println!("cargo:rustc-link-lib=ws2_32");

    // --- Langkah 5: Atur Rerun Triggers ---
    println!("cargo:rerun-if-env-changed=OPENSSL_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp_crypto/crypto_api.h");
    println!("cargo:rerun-if-changed=cpp_crypto/crypto_api.cpp");
}