> **INVALID (18 Nov review)** — semua hasil 0o / j1–j5 / "Jito-first", gradsim r1h/r6h/r24h, simulasi Kelly/posisi ($14 → $621.869), model $50k walk-forward (wf.txt, AUC 0,94–0,99), tes "forward" tim, dan kelas A/B/C sebagai daftar beli **TIDAK SAH**. Sebab: (A) harga masuk tak terjangkau (98% koin label lulus di slot create; deteksi kita p50 2,6 dtk), (B) fitur funding bocor label (has_fund saja AUC 0,93–0,99), (C) gradsim tak lihat crash & data AMM ~26 dtk bukan 1 jam. Angka jujur: reports/honest/.

# RINGKASAN HASIL - proyek jaringan dev (16 Nov 2026)
Semua angka dari perintah di VPS. Detail: checkpoints/STEP_LOG.md. Rencana fase: checkpoints/PLAN_OWNER.md.

## Data (P0-P2)
- Koin Pump.fun 9-15 Nov: ~290 ribu (+3 hari backfill, dijeda). Bersih: 0 duplikat, 0 urutan rusak, cek silang 200/200 cocok.
- 78,8% koin = template (nama+simbol sama dgn koin lain).
- Funder dev dicek: 11.143 wallet (target ~29 ribu, 14-15 Nov). Trade: 7.865 koin (target 27.700). Kredit Helius: 243 ribu / 1 juta.
- Data trade: 0 trade hilang, 9,5% koin terpotong (>500 trade).

## Beli semua koin (0,1 SOL, jual saat dev jual, fee termasuk)
- Telat 1-13 slot: menang 8-12%, rata-rata -6..-7% per trade. RUGI.
- Blok sama setelah sniper: -4..-5%. Pembeli pertama (Jito): +7,7%; setelah 1 sniper +2,4%; setelah 2 sniper -0,8%.
- Tip Jito teratas 0,00146 SOL (~1,5% modal).

## Sindikat / funder / tim (P3, sementara)
- 1.377 tim (1 funder = 1 tim), 123 dgn >=2 dev, 8 tim punya moonshot (>=10x).
- Sinyal ikut tim setelah moonshot: 114 koin, menang 18%, -0,7% (koin biasa +1,8%). BELUM ada tim lebih bagus dari koin biasa.
- Funder terbesar 8DfHuYFv (129 dev): 1000 tx dlm 0,8 jam = kemungkinan layanan/bot, bukan sindikat.
- Bundle cluster 4 wallet (3CiATcki, 52nK7jPY, BavuE4Kk, DG26djGJ): koin naik 2,25x rata-rata tapi pembeli rugi -42% -> HINDARI.
- Buang dev "ragpul" (jual <60 dtk): malah lebih jelek.

## Model CatBoost (P6, dicoba lebih dulu, dijeda)
- Telat 2 slot: AUC 0,79, top 10% tetap rugi -7%.
- Jito pembeli pertama: AUC 0,86, top 10% menang 70% +24% (tip termasuk).
- Jito setelah 1 sniper: AUC 0,86, top 10% menang 64% +14%, biaya x2 +10%.
- BELUM terbukti: uji baru 1 hari, kena aturan "terlalu bagus", posisi 1-2 di blok belum terbukti bisa didapat.

## Kesimpulan saat ini
- Tanpa jadi pembeli paling depan: tidak ada strategi untung.
- Ikut sindikat/tim: belum terbukti lebih baik.
- Satu-satunya untung: posisi 1-2 di blok (Jito + stream pra-blok, paket berbayar) - belum diuji di hari baru.

## Berikutnya
P1-P2 selesai (funder + trade 14-15 Nov) -> P3 daftar tim tanpa exchange/layanan -> P4 GOOD/BAD/TOO_FEW/UNKNOWN.
