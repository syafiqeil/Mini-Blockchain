# Mini Blockchain Implementation

Blockchain fungsional dengan arsitektur hybrid Rust-C++ untuk keamanan dan performa optimal. Implementasi mencakup jaringan P2P, konsensus PoA, penyimpanan RocksDB, dan antarmuka JSON-RPC.

## 🧠 Arsitektur Inti

| Komponen | Teknologi | Strategi Integrasi |
|----------|-----------|--------------------|
| **Core System** | Rust | Logika utama blockchain |
| **Kriptografi** | C++ (Botan/OpenSSL) | FFI via C ABI |
| **Jaringan P2P** | libp2p-rs | Modular networking |
| **Konsensus** | Proof-of-Authority | Validator terdaftar |
| **Penyimpanan** | RocksDB | Key-value persisten |
| **Antarmuka** | JSON-RPC | Warp/Actix-web server |

## ⚡ Fitur Utama

* **Hybrid Rust-C++**: Keamanan memori Rust + performa kripto C++
* **FFI Cerdas**:
  * Otomatisasi binding dengan `bindgen`
  * Build terintegrasi via `build.rs`
  * Wrapper aman untuk panggilan unsafe
* **Jaringan Terdesentralisasi**:
  * Discovery node dengan MDNS
  * Gossipsub untuk broadcast
* **Manajemen State**: 
  * RocksDB untuk penyimpanan persisten
  * State machine untuk transaksi

## Menjalankan Program

```bash
Terminal 1:
cargo run -- --is-authority --db-path ./database1

Terminal 2 :
# Cek jumlah blok
curl [http://127.0.0.1:8080/block_count](http://127.0.0.1:8080/block_count) 

# Kueri Blok Tertentu (Genesis Block)
curl [http://127.0.0.1:8080/block/0](http://127.0.0.1:8080/block/0)

