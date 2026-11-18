# Hasil penelitian & masalah — 18 Nov 2026

Ditulis untuk pemilik. Ringkas: apa yang ditemukan, apa artinya untuk trading, dan apa yang masih jadi masalah.
File data pendukung ada di `family_export/` (CSV) dan `data_gz/` (semua data mentah).

## A. File daftar wallet (folder `family_export/`, batch 1 = 443 dev DexScreener)
| File | Isi |
|---|---|
| `family_ringkas.csv` | 1 baris per family (97): induk, status PAKAI/KELUARKAN, aktif sejak 15 Nov, ujung atas rantai dana, jumlah wallet/dev/perantara, koin, mati di Pump.fun, lulus, di DexScreener, mcap ≥$40k/≥$50k/≥$100k/≥$1jt, rug, koin terbaik |
| `wallet_family.csv` | 1 baris per wallet (2.745): induk, alamat lengkap, peran, kedalaman dari induk, jumlah tx dibaca, koin yang dibuat |
| `koin_family.csv` | 1 baris per koin (402): induk, mint, simbol, pembuat, waktu, status, mcap |

**Arti peran wallet:**
- **INDUK** (97): wallet inti tempat rantai dana berakhir. Sering berupa "pengumpul" (modal awalnya debu, tapi mengirim ratusan–ribuan SOL).
- **PERANTARA** (206): wallet yang hanya meneruskan SOL dari induk ke dev (biasanya 2 transaksi: terima → kirim).
- **DEV** (181): wallet yang membuat koin. Hampir selalu sekali pakai (1–2 koin).
- **RAMAI** (703): wallet yang menerima dana family tapi transaksinya ratusan+ (bot, trader, layanan). Tidak ditelusuri lebih jauh.
- **KELUARGA** (1.558): wallet lain yang menerima ≥ 0,05 SOL dari family.

Batch berikutnya (semua 4.673 dev) sedang berjalan; file ini akan diganti versi lengkap.

## B. Temuan utama
1. **Dev itu sekali pakai.** Dev membuat 1–2 koin lalu ditinggal. Yang membuat koin berikutnya adalah induknya lewat wallet dev BARU. → Yang harus dipantau adalah **induk dan aliran dananya**, bukan dev.
2. **Pola tim besar (COGE, GTA6, BLACKX, SPAX10R, AODT, juga GODF/UDTF/U0DF/USEF):** dev baru diisi 100–1.018 SOL lewat 4–6 lapis perantara, 10–220 detik sebelum koin lahir. Di blok lahir yang sama (satu bundle): create + borong seluruh curve + pindah ke PumpSwap + tim beli 400–774 SOL → harga ×30–100.
3. **Satu operator, banyak induk.** Ticker gaya kode saham AS tersebar di beberapa induk yang saling terhubung (contoh AKEaFo9m menyambung GNgDt5Gf & 6XARUUbJ; 3HfMFfeB membeli COGE & BLACKX).
4. **Pola family: satu koin raksasa, sisanya mati.** Contoh w7ziqBqH: 36 koin, 29 mati di Pump.fun, 1 moonshot $5,6 jt.
5. **Koin family yang mati (97 terbaca):** RUG 17 (18%), SEPI 28 (29%), LEMAH 52 (54%), naik-lalu-mati 0.
6. **Aturan pemilik diterapkan** (sering rug = ≥2 rug & ≥1/3 koinnya, kecuali ≥2 koin ≥$100k): 94 family dipakai, 3 dikeluarkan (contoh 2t1DmPSw: 14 koin, 8 rug, masih aktif).
7. **Riwayat dev (batch 1):** dari 930 koin buatan dev, 75% mati di Pump.fun; ≥$40k 11,4%; ≥$50k 10,3%; ≥$100k 7,7%; ≥$1jt 3,4%.

## C. Hasil uji strategi (jujur)
| Uji | Hasil | Arti |
|---|---|---|
| Beli 5 koin tim besar di menit ke-1, tahan | 1,1–2,0x; $100 → $182 | bias: 5 koin dipilih karena sudah menang |
| Jadi pembeli pertama di koin model tim besar | **tidak mungkin** | semua terjadi di 1 bundle; bot tercepat (27HFmP7c) pun dapat harga ×30–100 |
| ML lulus/mati (fitur transaksi create) | AUC 0,80; top 0,1% presisi 90% | bagus menebak "lulus" |
| ML top 2% + beli < 1 detik, jual saat lulus | median 0,73x, 14% untung, 1/4 Kelly = 0 | **tidak untung**: koin yang pasti lulus sudah dibeli besar dev, harga kita sudah dekat harga lulus |
| ML top 2% + tahan s/d 18 Nov | median 0,56x | tidak untung |

**Kesimpulan saat ini:** belum ada cara masuk yang terbukti untung. Peluang yang tersisa:
- koin family yang lahir **biasa** (tidak diborong saat lahir) → kita bisa jadi pembeli awal;
- sinyal awal: dana ±100 SOL masuk ke wallet baru lewat perantara dari induk BAGUS → siap beli saat create;
- ML kedua yang menebak **untung** (bukan sekadar lulus), dilatih khusus koin family.

## D. Masalah
1. **VPS:** ssh langsung & semua port langsung instance rusak (koneksi diterima lalu diputus). Hanya proxy `ssh7.vast.ai:28647` yang jalan, dan lambat (±60 KB/detik). Penyebab awal: terlalu banyak job berat berjalan bersamaan.
2. **Bias pemilihan:** family ditemukan dari koin yang SUDAH sukses di DexScreener, jadi angka sukses family terlihat lebih tinggi dari aslinya. Uji jujur harus: pilih induk dari data lama, uji di koin baru.
3. **Riwayat dev terpotong:** pembacaan riwayat dari transaksi terbaru melewatkan koin lama dev yang ramai. Sedang diulang dari transaksi tertua untuk semua 4.673 dev.
4. **Koin quote non-SOL:** decoder belum membaca harga koin yang pasangannya bukan SOL; rug untuk koin itu dinilai dari porsi jual family saja.
5. **Data candle/ATH terbatas:** pemilik melarang unduh grafik; ATH hanya ada untuk koin yang candle-nya sudah tersimpan.
6. **Kredit Helius:** terpakai ±360 rb dari batas 1 jt; job penuh diperkirakan ±500 rb lagi.
7. **Snapshot DexScreener 18 Nov:** koin yang lahir 17 Nov dinilai saat masih "hidup" → terlihat lebih bagus dari kenyataan.

## E. Sedang berjalan (VPS, berurutan)
1. Riwayat semua 4.673 dev dari transaksi tertua.
2. Family: sisa dev ditelusuri ke atas, lalu semua family ditelusuri ke bawah (maks 10.000 wallet).
3. Koin mati → rug/sepi; ringkasan; ekspor semua wallet → diunggah ke folder `family_export/`.
