# PLAN V3 — Watchlist -> prediksi -> beli saat create (pemilik 21 Nov 2026)

Tujuan pemilik: pantau wallet entitas berkualitas (family digabung jadi 1 / creator independen). Saat entitas watchlist
membuat koin baru -> beli secepat mungkin saat create (sebelum koin tampil di web Pump.fun / DexScreener) -> tahan
sampai koin masuk DEX. ML memprediksi apakah koin baru entitas itu akan naik lagi dan berapa persen.

## Fakta yang mengikat desain
- 78% creator DexScreener yang riwayatnya lengkap hanya membuat 1 koin (wallet sekali pakai). Koin berikutnya datang
  dari wallet baru yang didanai INDUK -> pantau INDUK/PERANTARA, bukan hanya wallet creator.
- Koin tidak bisa dibeli sebelum tx create ada di chain. Paling cepat: blok yang sama/berikutnya (slot+0/+1),
  yaitu sebelum muncul di web Pump.fun/DexScreener. 18 Nov: deteksi kita via RPC median 2,6 dtk -> hasil jujur saat itu TANPA EDGE.
- Pelajaran 18 Nov: fitur funding bocor ke label (AUC 0,93-0,99 palsu). Semua fitur wajib tersedia SEBELUM/SAAT create.

## Langkah
1. Data lengkap (butuh kredit Helius): riwayat + ATH 3.611 creator DS (restart_v7.sh), family (fam2), watchlist penuh.
2. Dataset per peluncuran (urut waktu). Untuk tiap koin baru dari entitas yang SUDAH dikenal sebelum koin itu dibuat:
   - Fitur (hanya data <= saat create): reputasi entitas s/d hari itu (jumlah koin, % berhasil ATH>=$50k, % rug,
     median & max naik, % sepi, cara exit dev), jam sejak koin terakhir, SOL dari induk ke wallet baru, umur wallet,
     dev buy di tx create, jam/hari.
   - Label: (a) koin lulus ke DEX? (b) naik maks dari harga masuk TERJANGKAU (slot+1 dan +2 dtk, bukan harga launch),
     (c) hasil strategi: beli slot+1, tahan sampai DEX / stop / dev dump -> % untung-rugi bersih fee.
3. Uji: walk-forward. Latih minggu lama, setel di minggu validasi, uji SEKALI di minggu terbaru (tak disentuh).
   Pembanding: aturan reputasi sederhana tanpa ML. ML hanya dipakai jika mengalahkan pembanding di uji.
   Model: CatBoost (C++) / gradient boosting di Rust; tanpa Python.
4. Ukuran sukses (strategi positive-skew): rata-rata % per trade dari harga terjangkau, total %, distribusi (berapa
   koin >= 10x/100x), drawdown, simulasi 1/4 Kelly. Hit-rate/AUC saja tidak cukup.
5. Eksekusi cepat: pre-subscribe wallet baru yang baru didanai INDUK; feed latensi rendah (ShredStream / Geyser gRPC);
   kirim beli lewat Jito bundle ke slot+1; ukur latensi nyata di mode paper (modul shadow).
6. Paper/shadow dulu. Live hanya dengan kata tegas pemilik.
7. Laporan akhir (artifact): watchlist + reputasi + hasil uji ML + total % naik + latensi; CRITIC (agent) di akhir.

## Butuh keputusan pemilik
- Kredit Helius (sekarang "max usage reached").
- Feed latensi rendah untuk beli saat create (berbayar: Helius LaserStream/Triton Geyser; Jito ShredStream perlu daftar).
