# Modul Rust 18 Nov

Salinan modul yang dipakai di VPS (`/root/jaringan/src/`). Versi lengkap (beserta `main.rs` yang sudah
menyambungkan perintahnya dan `fund.rs` dengan `pub fn one`) ada di VPS dan akan diunggah saat VPS hidup.

Cara pasang manual: salin ke `src/`, tambahkan `mod family; mod sim5; mod entry1s; mod famsel; mod deadcls; mod gradml;`
di `main.rs`, dan baris perintah (lihat tabel di `STATUS.md`), lalu `cargo build --release`.
