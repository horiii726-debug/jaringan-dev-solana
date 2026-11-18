# Hasil ML v3 (jujur) — 18 Nov 2026

Model CatBoost menebak, saat kita beli ±1 detik setelah koin lahir: **MATI** (mati di Pump.fun, tidak pernah +30%),
**SCALP** (naik ≥ 30% lalu mati), **LULUS** (masuk DEX), **MOON** (lulus & ≥ 10x). Desain: lihat `LANJUT_ML_AUDIT.md`.
File lengkap: `data_baru_18nov/ml3/` (report, rows, prediksi). Kode: `src/ml3.rs`, `ml3_build.sh`, `ml3_train.sh`.

## Data
- Semua koin SOL 14–15 Nov: 28.217 (lulus 1,94%). Yang punya data trade: 22.213 (lulus 1,92%) → **sampel representatif**.
- Masuk berhasil 18.913, gagal isi 3.300 (tidak ada trade setelah latensi).
- Latih (14 Nov 00–18): MATI 7.304 | SCALP 1.158 | LULUS 174 | MOON 10.
- Validasi (14 Nov 20–24): MATI 2.154 | SCALP 317 | LULUS 58 | MOON 2.
- Uji (15 Nov, dilihat sekali): MATI 5.422 | SCALP 855 | LULUS 136 | MOON 9.

## Kebersihan (anti-bocor)
- Loss uji model **0,3827** vs tebak-rata-rata 0,5023 → model **23,8% lebih baik** dari tebakan rata-rata.
- **Uji label diacak: 0,5003 vs 0,5023 (+0,4%)** → tidak ada kebocoran lewat pipeline. ✅
- Fitur terpenting: skor lulus model 7–12 Nov, isi curve saat kita lihat (seen_vsol), tingkat lulus pasar 6 jam,
  beli terbesar di detik pertama, total beli di detik pertama.

## Seberapa tajam
| Kelas asli (uji) | Jumlah | Ditebak benar |
|---|---|---|
| MATI | 5.422 | 5.405 (99,7%) |
| SCALP | 855 | 15 (1,8%) |
| LULUS | 136 | 43 (32%) |
| MOON | 9 | 0 |
Model hebat mengenali koin mati, lumayan untuk "lulus", tapi **hampir tidak bisa** mengenali SCALP dan MOON.

## Uang (beli ±1 detik, $100, 1/4 Kelly, compounding)
- Aturan dipilih di validasi: P(tidak MATI) top 2%, exit TP +500% / SL −15% / maks 15 detik.
  - Validasi: 50 trade, rata +11,3%.
  - **Uji: 79 trade, menang 12,7%, rata −6,0%, median −5,8% → $100 jadi $51 (drawdown 58%).**
- Pembanding tanpa ML (beli semua koin): rata −6,8%.
- Blok masuk: paling cepat blok +1, median **blok +4**, 90% ≤ blok +7; separuh lewat pool (curve sudah habis).

## Diagnostik (BUKAN strategi — kalau kelasnya sudah diketahui)
- Koin SCALP + TP 30%: rata **+36,8%**, menang 96,7%.
- Koin LULUS + TP 200% / SL 50%: rata **+219%**.
- Koin MATI, exit terbaik pun: rata −9,3%.
→ Semua untung ada di **mengenali SCALP/LULUS sebelum beli**. Data detik pertama + transaksi create belum cukup.

## Koin family di data uji
- Family yang dikenal sebelum 14 Nov: **0 koin** di hari uji yang punya data trade; semua family: 2 koin (−6,8%).
  → strategi "hanya beli koin family" **belum bisa diuji** dengan data ini (perlu tarik trade koin family baru).

## Kesimpulan
Model sudah bersih (tidak bocor) dan lebih baik dari tebakan rata-rata, tapi **belum menghasilkan untung** di hari uji.
Langkah berikut yang masuk akal:
1. Tarik data trade untuk semua koin baru family (setelah family dikenal) → uji strategi family secara jujur.
2. Tambah fitur detik 1–5 (masuk lebih lambat sedikit tapi info lebih banyak) untuk mengenali SCALP.
3. Latih per kelompok: koin yang curve-nya sudah habis (masuk lewat pool) dipisah dari koin curve.
