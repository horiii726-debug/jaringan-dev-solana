# Ringkasan V3 — reputasi creator, watchlist, ShredStream (21 Nov 2026)

Data hasil lama (dataset/, data_baru_18nov/, reports/) sudah dihapus dari repo: banyak yang dinyatakan tidak sah
pada review 18 Nov dan ATH-nya memakai rumus yang salah. Riwayatnya tetap ada di git history.
Data sekarang ada di `data/v3/` (lihat di bawah).

## Tujuan
Kumpulkan creator (dev) + family-nya dari koin yang ada di DexScreener -> nilai reputasinya (koin naik >= $50k,
jarang rug, tidak jual di detik awal, koin tidak sepi) -> watchlist -> pantau wallet (DEV + INDUK + PERANTARA)
-> saat entitas watchlist membuat koin baru, beli secepat mungkin (slot yang sama/berikutnya) dan tahan sampai DEX.
Rencana ML & eksekusi: `checkpoints/PLAN_V3_PREDIKSI.md`.

## Perbaikan penting
1. Rumus ATH PumpSwap salah 26% (terlalu rendah): event Buy/Sell sekarang punya `virtual_quote_reserves` dan
   `base_supply`. Harga = (quote + virtual quote) / base x supply. Tervalidasi vs GeckoTerminal +-5%.
2. 14% kelulusan dipasangkan dengan USDC / token lain, bukan SOL -> dulu terbaca ATH palsu jutaan dolar
   (mis. SPONGEBOB "$9,5 juta"). Sekarang hanya pool berpasangan SOL yang dipakai.
3. ATH terkonfirmasi: harga harus bertahan 3 transaksi berturut-turut; event dengan data tak wajar dibuang;
   cek silang harga hitung vs harga eksekusi trade kecil (DATA_RAGU kalau sering meleset).
4. Koin "berhasil" palsu dibuang: pool sepi (< 200 transaksi / < 50 trader) dan pool cuma-beli (jual < 5%).
5. 7,8 ribu creator sempat tidak pernah ditelusuri (yatim di roles.jsonl) -> devtrace v6/v7 mengantrekannya.
6. Saat kuota Helius habis, devtrace/coinrise dulu menulis baris kosong yang dianggap selesai -> sekarang berhenti;
   baris rusak dibuang.
7. Penulisan file paralel dibuat satu kali tulis per baris (tidak ada baris tercampur antar proses).
8. Laju diatur sesuai batas key Helius (~15 panggilan/detik); Shyft + Tatum + RPC publik untuk panggilan ringan.

## Aturan saringan (config/reput.toml)
- Hanya creator yang koinnya ada di DexScreener.
- ELIMINASI: rug > 40% koin terukur | >= 50% koin sepi | jual pertama <= 10 detik pada >= 50% koin.
- Berhasil = ATH >= $50k (bukan sepi / palsu). Family digabung jadi 1 entitas (semua wallet DEV dinilai bersama).
- Watchlist = entitas lolos + >= 1 koin berhasil + aktif <= 30 hari. Wallet dipantau: DEV + INDUK + PERANTARA
  (78% creator memakai wallet sekali pakai; koin berikutnya datang dari wallet baru yang didanai induk).

## File data (data/v3/)
- `reput/watchlist.json|tsv` — watchlist + wallet yang dipantau; `reput/watchlist_koin.tsv` — arsip proyek tiap entitas.
- `reput/entitas.tsv` — semua entitas + status/alasan; `reput/dev_reputation.tsv`, `creator_coins.tsv` — per creator.
- `ath/ath_*.jsonl` — ATH per koin (formula 3); `dev/` — riwayat dev (devtrace); `fam2/`, `view/` — family.
- `universe/` — koin DexScreener + kelulusan on-chain.

## Program (Rust)
- `src/reput.rs` (rapor + saringan + watchlist), `src/athgt.rs` (ATH dari trade pool), `src/devtrace.rs` (riwayat dev),
  `shredrx/` (penerima ShredStream gRPC: deteksi create Pump.fun ~0,15-0,5 dtk setelah blok, cocokkan watchlist).
- Jalankan semua: `bash start_all.sh` + `bash start_shredrx.sh`; hentikan: `bash stop_helius.sh`.
