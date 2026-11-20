# jaringan-dev-solana — V3 (creator & family dev Pump.fun dari koin DexScreener)

Angka terbaru (diperbarui tiap commit): **[STATUS_V3.md](STATUS_V3.md)**. Rincian perbaikan: [RINGKASAN_V3.md](RINGKASAN_V3.md).
Rencana ML + eksekusi: [checkpoints/PLAN_V3_PREDIKSI.md](checkpoints/PLAN_V3_PREDIKSI.md). Titik lanjut kerja: [checkpoints/STATE.md](checkpoints/STATE.md).

Semua data hasil lama (sebelum 20 Nov: belum bersih dari exchange/bot, ATH rumus lama) sudah dihapus dari repo.
Yang ada sekarang hanya data V3 di `data/v3/`.

## Tujuan
1. Kumpulkan creator (dev) koin Pump.fun yang ada di DexScreener, telusuri semua koin yang pernah dia buat.
2. Gabungkan wallet milik orang yang sama jadi 1 family (induk pendana -> wallet dev). Exchange, bot, akun program dibuang.
3. Nilai reputasi: koin berhasil (ATH >= $50k, bukan pool sepi/palsu), rug, koin sepi, kapan dev mulai jual.
4. Watchlist = family/creator yang lolos -> pantau DEV + INDUK + PERANTARA -> saat membuat koin baru, beli
   secepat mungkin (blok yang sama/berikutnya) dan tahan sampai DEX. Paper dulu; live hanya atas perintah pemilik.

## Penemuan
- **78% creator hanya membuat 1 koin** (wallet sekali pakai). Koin berikutnya dibuat wallet baru yang didanai induk,
  jadi yang harus dipantau adalah INDUK, bukan hanya wallet creator.
- **Rumus ATH PumpSwap lama 26% terlalu rendah**: pool sekarang punya cadangan SOL virtual + suplai token di event.
- **14% kelulusan dipasangkan USDC / token lain**, bukan SOL -> dulu terbaca ATH palsu jutaan dolar (SPONGEBOB "$9,5 jt").
- **ATH raksasa palsu**: pool sepi (USEF "$97 jt" dari 58 transaksi) dan pool cuma-beli (EXEN 4.078 beli vs 42 jual).
- **Sebagian kelulusan resmi masuk pool kosong** (±0,15 SOL, harga ~0) -> koin itu gagal, bukan salah baca (dicek harga eksekusi).
- **Bersih dari exchange/bot**: wallet dicek satu per satu; exchange, bot/wallet super ramai, akun program tidak dipakai
  untuk menyambung family (jumlah di STATUS_V3.md).
- **ShredStream gRPC**: create koin Pump.fun terdeteksi ±0,15-0,5 detik setelah blok (RPC biasa: ±2,6 detik).
  Sebagian transaksi di stream memakai format transaksi baru Solana (diawali 0x81).
- **Key Helius baru dibatasi ±15 panggilan/detik**; lebih dari itu ditolak (HTTP 429).
- Server vast.ai **tidak bisa menerima UDP** -> ShredStream mentah (UDP) tidak bisa; gRPC bisa.

## Isi repo
| Folder / file | Isi |
|---|---|
| `src/` | program Rust `jaringan` (subcommand). V3 yang dipakai: `dexs`, `devtrace`, `fam2`, `famview`, `coinrise`, `athgt`, `reput`, `udpprobe`, `athdbg` |
| `shredrx/` | penerima ShredStream gRPC (deteksi create Pump.fun, cocokkan watchlist) |
| `config/` | aturan & ambang (`reput.toml` = saringan creator, `devtrace.toml`, `budget.toml` = batas kredit) |
| `data/v3/reput/` | `watchlist.json/tsv`, `watchlist_koin.tsv` (arsip proyek), `entitas.tsv` (family + independen), `dev_reputation.tsv` & `creator_coins.tsv` (per creator) |
| `data/v3/export/family_anggota.tsv` | semua anggota family: root, wallet, peran (DEV/INDUK/PERANTARA/PENERIMA) |
| `data/v3/view/` | family aktif <= 30 hari, dev independen, wallet sekali pakai |
| `data/v3/dev/` | riwayat koin tiap dev (`devs.jsonl`) + creator tiap koin (`roles.jsonl`) |
| `data/v3/fam2/` | sambungan dana antar wallet + kelas wallet (exchange/bot/biasa) |
| `data/v3/ath/` | ATH per koin (formula 3) |
| `data/v3/universe/` | koin DexScreener + kelulusan Pump.fun on-chain |
| `*.sh` | operasi: `start_all.sh`, `stop_helius.sh`, `start_shredrx.sh`, `reput_loop.sh`, `restart_*.sh` |

## Menjalankan (di VPS)
```
bash start_all.sh        # devtrace, fam2, coinrise, athgt (laju sesuai batas key Helius)
bash restart_reput.sh    # rapor + watchlist tiap 10 menit
bash start_shredrx.sh    # penerima ShredStream (butuh .allenhark.env)
bash stop_helius.sh      # hentikan semua job Helius
```
Kunci API hanya di file `.env*` di server (tidak pernah di-commit).
