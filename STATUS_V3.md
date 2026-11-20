# Status V3 — 2026-11-20 23:31 UTC

Angka di bawah dihasilkan otomatis dari data saat commit ini (`jaringan reput`, `jaringan famview`).
Pengumpulan data MASIH BERJALAN, angka akan naik di commit berikutnya.

## Creator (hanya yang koinnya ada di DexScreener)
- Creator: **4376** (dibuang karena bukan koin DexScreener: 5004)
- Riwayat belum ditelusuri: 3114
- Koin tercatat: 27255; koin dengan ATH terukur: 514; ATH >= $50k (asli): 169; >= $100k: 104; >= $1 jt: 16
- Pool kosong (lulus tapi harga ~0): 464; ATH data ragu: 0
- Status creator: BELUM_CUKUP_DATA 3140, ELIMINASI 614, LOLOS_BELUM_TERBUKTI 504, LOLOS_DATA_KURANG 118
- Alasan eliminasi: JUAL_DETIK_AWAL 163, KOIN_SEPI 441, SERING_RUG 73

## Family (satu operator = satu entitas)
- Family aktif <= 30 hari: **500**, wallet di dalamnya: 4565
- Peran wallet: DEV 670, INDUK 333, PENERIMA 3002, PERANTARA 560
- Dev independen aktif: 760; wallet dev sekali pakai: 211
- Kelas wallet yang dicek: INFRA_BUSY 269, INFRA_LIST 23, NON_WALLET 1162, ORDINARY 4089,  (INFRA_LIST = exchange, INFRA_BUSY = bot/wallet super ramai, NON_WALLET = akun program; ketiganya tidak dipakai menyambung family)

## Watchlist
- Entitas: **116** (family 26, independen 90)
- Wallet dipantau (DEV + INDUK + PERANTARA): **182**
- Koin di arsip watchlist: 414, di antaranya berhasil (ATH >= $50k): 122
- Status entitas: FAMILY:BELUM_CUKUP_DATA 40, FAMILY:ELIMINASI 186, FAMILY:LOLOS_BELUM_TERBUKTI 155, FAMILY:LOLOS_BERKUALITAS 2, FAMILY:LOLOS_DATA_KURANG 24, INDEPENDEN:BELUM_CUKUP_DATA 3091, INDEPENDEN:ELIMINASI 378, INDEPENDEN:LOLOS_BELUM_TERBUKTI 294, INDEPENDEN:LOLOS_DATA_KURANG 90

## Sudah dikerjakan
- Pengumpulan creator & family dari koin DexScreener, saringan (rug / sepi / jual detik awal), watchlist + arsip proyek.
- ATH formula 3 (tervalidasi +-5% vs GeckoTerminal), buang pool non-SOL, pool sepi, pool cuma-beli.
- Penerima ShredStream gRPC: deteksi create Pump.fun +-0,15-0,5 dtk setelah blok, cocokkan watchlist + wallet baru kiriman induk.
- Data rusak akibat kuota Helius dibersihkan; job berhenti saat kuota habis; laju sesuai batas key.

## Belum dikerjakan
- Data belum lengkap (ATH & riwayat dev masih berjalan; lihat angka "belum ditelusuri").
- Mode paper (beli pura-pura saat watchlist membuat koin, tahan sampai DEX, hitung untung/rugi & latensi).
- Uji historis + ML (prediksi koin baru entitas akan naik lagi), walk-forward, pembanding aturan sederhana.
- Alamat mint untuk transaksi format baru (0x81) di stream belum terbaca (nama, simbol, creator sudah).

## Masalah
- Key Helius dibatasi +-15 panggilan/detik (lambat); sisa kredit harus dicek di dashboard Helius.
- Saldo vast.ai rendah: kalau habis, server berhenti.
- Trial ShredStream habis 21 Nov; uji panjang butuh paket berbayar. Server vast tidak bisa menerima UDP.
- VPS tidak punya login GitHub (push lewat laptop).
- Belum ada entitas watchlist yang membuat koin selama uji ShredStream (pencocokan belum terbukti dengan contoh nyata).
