# Lanjutan 18 Nov malam — ML v3 + audit kode (STATUS SAAT DISIMPAN)

## Sudah dikerjakan
- **ML v3** (`src/ml3.rs`, perintah `jaringan ml3 build|eval`): model jujur "beli ±1 detik setelah koin lahir → MATI /
  SCALP (naik ≥30% lalu mati) / LULUS / MOON (lulus & ≥10x)" + pilih exit (TP/SL/batas waktu, 128 kombinasi).
  - Populasi: semua koin SOL 14–15 Nov yang punya data trade (tr_all + tr_rand) — tidak dipilih dari hasil.
  - Masuk = `honest::entry` (trade pertama ≥ 1 dtk setelah create, slot > slot lahir, kita masuk SESUDAH trade itu;
    koin yang curve-nya habis di blok lahir masuk lewat pool). Gagal isi dihitung.
  - Fitur hanya yang terlihat sebelum keputusan: transaksi create, trade dgn blockTime < lahir+1 dtk, riwayat creator/
    payer yang sudah terjadi, tingkat lulus pasar 6 jam, skor lulus model 7–12 Nov. Tanpa data funding (pernah bocor).
    Harga masuk BUKAN fitur.
  - Exit: pemicu terlihat di satu trade → jual terisi di kondisi pasar 1 dtk kemudian; fee + tip + biaya tx di dua sisi;
    data habis = CENSORED (dihitung, tidak dirata-rata).
  - Pembagian waktu: latih 14 Nov 00–18, embargo 18–20, validasi 20–24, uji 15 Nov (sekali).
  - Uji kebocoran: model kedua dengan label diacak harus sama dengan tebak-rata-rata.
- `ml3_build.sh` (bangun data) dan `ml3_train.sh` (latih CatBoost GPU + shuffle check + evaluasi, laporan:
  `data/ml3/report_family_dikenal.txt`, `report_family_semua.txt`, `shuffle_check.txt`).
- Daftar koin family untuk evaluasi: `data/ml3/fam_known.txt` (family yang sudah punya koin lulus sebelum 14 Nov =
  jujur) dan `fam_all.txt` (semua, dipilih dengan melihat ke belakang = pembanding saja).

## Sedang berjalan saat disimpan (VPS)
1. `ml3_build.sh` → `data/ml3/build.log` (tunggu baris ML3_BUILD_DONE), lalu jalankan `./ml3_train.sh`.
2. `pipeline.sh` (riwayat semua 4.673 dev dari transaksi tertua → family penuh → rug → ekspor wallet) → `data/family/pipeline.log`.
3. Audit kode (workflow, 16 pemeriksa + verifikasi 2 lawan per temuan) — hasil belum keluar saat disimpan.

## Bug yang sudah diketahui (perbaiki saat lanjut)
- `src/entry1s.rs` & `src/sim5.rs`: reserve event PumpSwap diperlakukan sebagai SESUDAH trade, padahal SEBELUM
  (lihat `honest.rs` `P::post`). Pakai `honest::P::post()`.
- `entry1s.rs` skenario "JUAL DI PUNCAK CURVE" = lookahead (menjual di puncak yang baru diketahui belakangan) → hapus.
- `entry1s.rs` "jual saat lulus" memakai harga lulus konstan 4,16e-7 tanpa latensi jual; harga keluar & biaya tidak realistis.
- Hasil simulasi family (median 1,38x, $100 → $42 rb / $505 rb) BIAS: koin penemu family ikut dihitung.
- `gradml.rs` menebak "lulus", tapi koin skor tertinggi = koin yang hampir diborong dev di transaksi create
  (sudah mahal saat bisa dibeli) → presisi tinggi tidak berarti untung.

## Cara melanjutkan
```
ssh -p 28647 root@ssh7.vast.ai          # ssh langsung 66.154.125.122:40327 rusak
cd /root/jaringan
tail -3 data/ml3/build.log; ./ml3_train.sh; cat data/ml3/report_family_dikenal.txt data/ml3/shuffle_check.txt
tail -3 data/family/pipeline.log
```
