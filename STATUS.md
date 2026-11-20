> DIGANTIKAN (21 Nov): versi 18 Nov; angka & status terbaru di STATUS_V3.md dan README.md

# STATUS jaringan-dev-solana — 18 Nov 2026

Laporan kerja 18 Nov (sesi "family / induk"). Ditulis untuk pemilik dan untuk sesi berikutnya.

> **Repo ini = salinan penuh VPS per 18 Nov sore** (riwayat lama dihapus total atas permintaan pemilik).
> - Kode: semua file `/root/jaringan` (Rust `src/`, skrip, `config/`, `checkpoints/`, `reports/`, `dataset/`).
>   Modul 18 Nov ada di `src/` (salinan juga di `modul_18nov/`).
> - Data: SEMUA `/root/jaringan/data` (Helius, Bitquery, Shyft, DexScreener, family, ML) dikompres di
>   `data_gz/data.tgz.p_*`. Gabung & buka: `cat data_gz/data.tgz.p_* | tar -xzf -`
> - Tidak diunggah: `.env`, `secrets/`, `target/`, `logs/`. Pindai kunci: bersih.

## 1. Tujuan (kata pemilik)
Cari **induk** (wallet inti) dari dev koin yang sampai DexScreener. Pantau induk itu. Begitu salah satu wallet
dari induk membuat koin baru, **langsung beli** secepat mungkin. Induk harus bagus: banyak koinnya sampai
DexScreener dan naik (ratusan ribu – jutaan dolar). Induk yang sering rug dikeluarkan, kecuali ia juga banyak
membuat koin besar. Eksekusi pakai Rust (bukan Python), modal uji $100, 1/4 Kelly, compounding.

## 2. Sudah dikerjakan

### Data (di VPS, belum di GitHub)
| Data | Isi | Lokasi VPS |
|---|---|---|
| Peluncuran Pump.fun 6–18 Nov | semua event create (± 40 rb/hari) | `data/raw/launches_*.jsonl`, `data/launches_*.jsonl` |
| Koin lulus 1 Okt–18 Nov | 44.057 koin + pool + creator | `data/roots/pool_creators.jsonl`, `grad.tsv`, `mig_all.tsv` |
| DexScreener | 5.992 koin masih tercantum (mcap, likuiditas) | `data/roots/ds.jsonl` |
| Dev DexScreener | 4.673 dev unik | `data/family/devs.txt` |
| Funding dev | first funder, label exchange/bot/operator | `data/roots/*fund*.jsonl`, `labels.tsv`, `svc.jsonl` |
| Profil 819 koin hidup | inflow/outflow dev | `data/roots/alive_devprof.jsonl` |
| Candle (GeckoTerminal, lama) | ±110 koin | `data/roots/gt/` |
| Family batch 1 (443 dev) | rantai ke induk, wallet keluarga, koin keluarga | `data/family_a/` (`up.jsonl`, `wallets.jsonl`, `edges.jsonl`, `creates.jsonl`, `report.txt`, `rank.txt`) |
| Family penuh (4.673 dev) | sedang jalan saat VPS mati (±450 dev selesai naik) | `data/family/` |
| ML lulus/mati | tabel fitur + model CatBoost | `data/gml/` |
| Bitquery 16–18 Nov | transfer wallet tim | `data/fwd*/` |
| Cluster pemegang (Bubblemaps) | BSsm, Hspr | `data/holders/` |

### Kode Rust baru 18 Nov (salinan di folder `modul_18nov/` repo ini)
| Modul | Perintah | Fungsi |
|---|---|---|
| `family.rs` | `jaringan family <devs> <dir> <max>` / `family_report <dir>` | naik dari dev ke induk (berhenti di exchange/bot/wallet ramai/induk pengumpul), turun dari induk ke semua wallet keluarga (penerima dibaca dari perubahan saldo karena relay lewat akun sementara) + koin yang dibuat; hanya family asal jelas |
| `sim5.rs` | `sim5`, `instant`, `first_buyer`, `first_buyer_real` | simulasi 5 koin besar, pola "lulus instan", skenario pembeli pertama |
| `entry1s.rs` | `entry1s <list> <curve_trades> <amm_trades>`, `devsum` | beli < 1 detik (slot +2) lalu jual saat lulus / puncak / tahan; $100 1/4 Kelly; riwayat koin per dev |
| `famsel.rs` | `famsel <dir> <cut>` | peringkat induk; induk BAGUS dipilih dari data sebelum cut, diuji sesudah cut |
| `deadcls.rs` | `famdead <dir>`, `deadcls <dir> <trades>` | koin family mati: RUG / NAIK_LALU_MATI / SEPI / LEMAH + aturan pemilik + status aktif |
| `gradml.rs` | `gradml build <dir>`, `gradml eval <test> <pred>` | ML lulus/mati dari fakta transaksi create saja |

Juga diubah di VPS: `fund.rs` (`one` jadi `pub`), `main.rs` (perintah baru), `config/budget.toml`
(batas kredit Helius 1 juta, izin pemilik).

## 3. Hasil (jujur)

### Pola 5 koin besar 14–17 Nov (COGE, GTA6, BLACKX, SPAX10R, AODT)
- Dev = wallet baru diisi 100–1.018 SOL lewat 4–6 lapis wallet perantara (2 transaksi: terima lalu teruskan).
- Induk ketemu: **6XARUUbJ** (COGE), **Bui98JUc** (BLACKX), **HVPhTGo8** (SPAX10R); GTA6 & AODT berujung di exchange.
- **3HfMFfeB** = wallet pembeli tim (COGE 774 SOL, BLACKX 464 SOL), didanai keluarga 6XARUUbJ → COGE & BLACKX satu tim.
- Di **slot lahir yang sama** (satu bundle): create + borong seluruh curve + pindah ke PumpSwap + tim beli 400–774 SOL → harga ×30–100.
  Tidak ada orang luar masuk di sela-sela; bot tercepat (27HFmP7c) selalu dapat harga sesudah pompa.
  **Kesimpulan: koin model ini tidak bisa kita jadi pembeli pertama.** Masuk paling cepat blok +1.
- Beli tutup menit ke-1 lalu tahan: 1,1–2,0x per koin; $100 → $182 (5 koin terpilih = bias pemenang).

### Family batch 1 (443 dev DexScreener)
- 97 family asal jelas (±22% dev). 336 dev dibuang (dana langsung dari exchange/bot atau wallet ramai).
- Pola umum: **satu koin raksasa, sisanya mati**. Contoh: w7ziqBqH 36 koin, 29 mati di Pump.fun, 1 moonshot $5,6 jt.
- Moonshot per family: AH1YZod3 GODF $25,7 jt; 8YaS3RMU UDTF $17,7 jt; GNgDt5Gf U0DF $17,2 jt; CeycTrPZ USEF $10,2 jt.
- Ticker "kode saham AS" (GODF, UDTF, U0DF, USEF, UODF, COGE…) tersebar di banyak induk yang saling terhubung
  (contoh AKEaFo9m → GNgDt5Gf & 6XARUUbJ) → kemungkinan satu operator, perlu digabung jadi super-family.
- Dev hampir selalu **sekali pakai** (1–2 koin) → yang dipantau harus induk, bukan dev.

### ML lulus/mati (CatBoost GPU, fitur transaksi create saja)
- Latih 7–12 Nov (84.102 koin, 2,74% lulus), uji sekali 13–16 Nov (60.950 koin, 1,83% lulus).
- Logloss uji 0,0863 vs tebak-rata-rata 0,0915 (lebih baik 5,7%), AUC 0,80 (validasi 0,88 → turun di minggu baru).
- Skor tertinggi 0,1% (61 koin): 90% lulus; 1%: 20,5%; 2%: 19%.
- Fitur terpenting: riwayat wallet pembayar, beli awal dev, panjang nama, tingkat lulus pasar 6 jam.
- Catatan: "lulus" ≠ untung. Simulasi untung top 2% (beli < 1 detik) belum selesai.

### Skenario lain (untuk catatan)
- Pola "lulus instan" (dev borong curve saat lahir) 6–18 Nov: 2.090 koin, 2,0% ≥ $1 jt vs lulus biasa 0,04%.
- Pembeli pertama hipotetis semua koin itu: modal tetap $1/koin untung besar, tapi $100 + 1/4 Kelly **bangkrut**
  sebelum pemenang datang (97% mati, pemenang menumpuk di akhir periode + bias waktu snapshot).

### Hasil sesudah VPS kembali (18 Nov sore)
**Koin family yang mati di Pump.fun** (97 dari 286 punya data transaksi): RUG 17 (18%) | SEPI 28 (29%) |
LEMAH 52 (54%) | NAIK_LALU_MATI 0. Induk paling sering rug: Cyfxw8UL (5), CqwCdPMS (4), A3mHfEZp (3).
Induk paling banyak koin sepi: BRSkJtCy (10 sepi + 9 lemah).

**Riwayat dev (devhist, 443 dev):**
- Semua dev yang punya koin terbaca: 195 dev, 930 koin → 75% mati di Pump.fun, 24% di DexScreener, 7,7% ≥ $100k.
- Dev asal jelas (family): 38 dev, 69 koin → 43% mati, 58% di DexScreener, 26% ≥ $100k.
  **Bias:** 38 dari 69 koin itu adalah koin awal yang memang dipilih dari DexScreener. Koin LAIN dev family
  (31 koin) hanya ±2 yang sampai DexScreener (±6%).

**Untung ML (beli < 1 detik, 2% skor tertinggi minggu uji, 1.107 koin):**
| Jual | Median | Naik | 1/4 Kelly $100 |
|---|---|---|---|
| saat lulus | 0,73x | 14% | Kelly 0 → tidak trading |
| puncak curve (timing sempurna, tak realistis) | 1,00x | 49% | $41.299 |
| tahan s/d 18 Nov | 0,56x | 6% | Kelly 0 → tidak trading |
Kesimpulan: model ML menebak "lulus" dengan baik (AUC 0,80) tapi **tidak menghasilkan untung** bila beli < 1 detik:
koin yang pasti lulus sudah dibeli besar oleh dev, jadi harga beli kita sudah dekat harga lulus. 112 koin yang
curve-nya habis sebelum kita masuk (perlu data PumpSwap) belum dihitung.

Kredit Helius terpakai ±360 rb dari 1 jt.

**Akses VPS:** ssh langsung (66.154.125.122:40327) tidak menjawab; pakai proxy `ssh -p 28647 root@ssh7.vast.ai`.

## 4. Belum dikerjakan / terputus karena VPS mati
1. Job family penuh 4.673 dev (baru ±450 selesai tahap naik; tahap turun belum).
2. Ringkasan per family: jumlah moonshot, rug, **masih aktif sejak 15 Nov atau tidak** (kode siap di `deadcls.rs`, hasil belum keluar).
3. Simulasi untung ML top 2% (transaksi curve 1.219 koin sedang ditarik).
4. Riwayat dev lengkap (`devhist`, 443 dev; baru ±150).
5. Gabung induk yang saling terhubung jadi super-family.
6. Uji jujur: induk BAGUS (data ≤ 12 Nov) → koin baru 13–18 Nov, beli < 1 detik, $100 1/4 Kelly.
7. Pemantau live Rust (sinyal: wallet baru diisi ±100 SOL lewat perantara dari induk) + paper trade.
8. ~~Unggah semua data VPS ke repo ini~~ — SELESAI 18 Nov sore.
9. Simulasi untung untuk 112 koin ML yang perlu data PumpSwap; ML kedua yang menebak UNTUNG (bukan sekadar lulus).
10. Job family penuh dilanjutkan pelan-pelan (satu job berat per waktu) via proxy ssh.

## 5. Pelajaran / aturan
- Jangan Python; analisis & eksekusi Rust (CatBoost CLI boleh).
- Jangan unduh grafik/candle; pakai data on-chain.
- Mulai dari koin DexScreener; transfer kecil/acak bukan keluarga; exchange/bot bukan founder.
- Jangan jalankan banyak job berat bersamaan di VPS (diduga penyebab VPS macet 18 Nov).
- Kredit Helius terpakai ±233 rb dari batas 1 jt (18 Nov).
