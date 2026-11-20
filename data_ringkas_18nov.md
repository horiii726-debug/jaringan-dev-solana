> DIGANTIKAN (21 Nov): versi 18 Nov; angka & status terbaru di STATUS_V3.md dan README.md

# Data ringkas 18 Nov (diselamatkan dari log sesi)

Ini BUKAN data lengkap. File data asli (`data/family_a/*.jsonl`, `data/family/*.jsonl`, `data/roots/*`,
Helius/Bitquery) masih di VPS dan akan diunggah saat VPS hidup. Alamat yang terpotong (8 karakter) memang
hanya tampil sepotong di log; versi lengkapnya ada di file VPS.

## 1. Rantai dana 5 koin besar (dev ← ... ← induk), 14–17 Nov
Format: `wallet(jumlah tx, SOL yang diterima dari wallet berikutnya)`

| Koin | Mint | Dev | Rantai ke atas | Induk / ujung |
|---|---|---|---|---|
| COGE | `ATWR4SqMvGaj1tz94DuK2fWDtdf4nEHS6XPkxDmpump` | `pyrGirKyp6g6HSsK3wECjTJNo…` | pyrGirKy(1271tx,110,85) ← 2R5MxiMA(2tx,110,85) ← 47GuoESj(54tx,310,85) ← 5vEYvyjg(2tx,310,85) ← 3CVZtnmc(4tx,1310,85) ← 3QwjDXCN(2tx,1310,85) ← GMmegi95(145tx,1312,28) ← CvKcS5FK(2tx,1312,28) ← 6XARUUbJ | **`6XARUUbJc2cCk4VcnPXuWfENB3DtEZecH2oiB6SDCqM6`** (induk pengumpul, 292 tx) |
| GTA6 | `A4BeFbdjGNK2KCHmyNQ3fLBSGcbSK6AKr37pEqBxpump` | `FrKGhL2zwzCQgoJ9MV7fJuYX…` | FrKGhL2z(744tx,100) ← 6Ajx7YyJ(2tx,100) ← AgVnSfL7(2tx,100) ← A3R1858c(2tx,100) ← 96JvcF7L | `96JvcF7LSHFH7XCMBRXBMRZwZMUM7JPR3rgY7znP22eo` ← exchange `5dxvr2A9TwMs1gnQ7gCpYiem9ArtcqmnuKHF76Yr2mT6` |
| BLACKX | `sTbXthDDqewmmnn1MFyhYHaeekYq229M7nBizqRpump` | `7sm7etMbKoR8wTiPnZwognr…` | 7sm7etMb(657tx,100,01) ← BDWHyuxW(2tx,100,01) ← H7RRFGwg(4tx,600,01) ← 2XRMmRcu(2tx,600,01) ← 4CjtHhzV(102tx,601,01) ← 6jvozhAt(2tx,601,01) ← Bui98JUc | **`Bui98JUcZRcYfRhmFRvXCPKoGxSspNCvuZeJuXUB3Xyp`** (induk pengumpul, 203 tx) |
| SPAX10R | `941dyEVrMt8uEmtABhXiJQ6yZSGqepumQZQwLYzpump` | `BZmbJvQ91noaSXmhXbUjGBa…` | BZmbJvQ9(657tx,100,01) ← 8LbywXxC(2tx,100,01) ← AnLQFtQA(4tx,600,01) ← 7zKoqbUk(2tx,600,01) ← 8tN6aK7T(102tx,601,01) ← B9Yha9BV(2tx,601,01) ← HVPhTGo8 | **`HVPhTGo85SYMpNYgRZ2sd1hWe4cjmBQsUz3hF9Hnep4L`** (induk pengumpul, 208 tx) |
| AODT | `2q33nHFNvbNM6D8MaGP9UEfwdvy6otMct6PWynMipump` | `DuuAWERJFhHMSitEwKnE2LehZZJGLgqQuxiipdKXpshD` | DuuAWERJ(1401tx,1018,78) ← GLqDXczG(2tx) ← 7pD3Xs8k(2tx) ← 3T9GcGkk(2tx) ← EqSwc29h | `EqSwc29hwKvDqRDGs9PX91hgGxNQj8P7iWMk2v8CLK3m` ← exchange `9RDPo8GiSFAEG8SRXuNopz7jK7z7T69FcG1nN7aqFFAc` |

Pool PumpSwap: COGE `2KqgGWEGSgwm1z37iTiJmhkH1UujF1eXFJj3j8nDmr3T`, GTA6 `7MZGJ4dG98mynpFxw2R9vqBwVMyJvMyGgn8RRMPmBk3D`,
BLACKX `HmmdbzxrmouqpZsYjGBzQTo5wTyh7WeEEFN6hup2kpdm`, SPAX10R `2f4pSzv5p93C32n8WCgA8SjDBpGC6hgJx8HKi1m8VcTT`,
AODT `5X7EsLJosfUAvXTLcnXZVndWa7p53pCPpJDFiqSH6Kam`.

### Beli tim di slot lahir (PumpSwap)
| Koin | Pembeli tim (slot 0) | SOL |
|---|---|---|
| AODT | DuuAWERJ (dev) | 1 + 399 |
| COGE | 3HfMFfeB | 0,987 + 773,9 |
| GTA6 | HLLKMFzD, 8stiUnEg, 8pLZWaXW, BhStaWxT, 63VJtFXe, 3erCQzs9, Q4rNbHGh | 88–99 per wallet (657 total) |
| SPAX10R | 35z8yCi8 | 0,987 + 463,973 |
| BLACKX | 3HfMFfeB | 0,987 + 463,973 |

Wallet penting: **3HfMFfeB** (pembeli tim COGE & BLACKX; didanai AVx5MajE di keluarga 6XARUUbJ: 999 / 499 / 1.499 / 799 SOL),
**27HFmP7c** (bot tercepat, beli debu di semua 5 koin), **AKEaFo9m** (menyambung induk GNgDt5Gf & 6XARUUbJ).

## 2. Family teratas (batch 443 dev DexScreener)
| Induk | Wallet | Koin | Mati di Pump.fun | Lulus | DexScreener | Koin terbaik |
|---|---|---|---|---|---|---|
| `w7ziqBqHBBcduCD7xbTD99zTFZJdgEUC59UcMmprwGh` | 138 | 36 | 29 | 7 | 3 | uni `jM9r9PJFyPaMEbhGZPbDeZJiysn2pfyz5TzdDs5pump` $5,57 jt |
| AH1YZod3… | 15 | 10 | 8 | 2 | 1 | GODF `fXwcDSnhPAiqv8b9vZ7EY6yKtZ2TfuvniiAABvZpump` $25,7 jt |
| 8YaS3RMU… | 5 | 3 | 1 | 2 | 2 | UDTF `hPQpJNrG4XGiFr2SgzCy1EMbUdgdgwj78a6tmhHpump` $17,7 jt |
| CeycTrPZ… | 112 | 3 | 1 | 2 | 2 | USEF `sFoTr4DqBN6DPaNyrNEBeG6oURdyif8ebzzNsd3pump` $10,2 jt |
| GNgDt5Gf… | 133 | 3 | 1 | 2 | 1 | U0DF `oZ5neAELPt86Ssb3SCeW8YeSxNuqR4cikWBc4d8pump` $17,2 jt |
| 6XARUUbJ… | 167 (uji) | 3 | 1 | 2 | 1 | COGE $3,6 jt; UODF⚡️ `zj7AKYXfGLxk9oRa13hs5HquZgjaCqDjUQ1HiKxpump` (lulus, hilang); XT O⚡️ `rDfxW6u8qKoGk3LNAa3KaZ8TW7qPzCnutMWcnUypump` (gagal) |

Induk BAGUS (kriteria data ≤ 12 Nov): `w7ziqBqHBBcduCD7xbTD99zTFZJdgEUC59UcMmprwGh`, `2t1DmPSwYRc3RmLjr6u4rLLSQPRviChYUpaG8Mi8U424`,
`3GaqEe5Ly3tBBtUdYL6wvfZxyFtB6LXcj5HcFbLo9Dnf`, `7jq6vReJ3FmwBuDhUxxzYdUTvW51LwRrjBH98eRHgrGB`, `8nyRizUwUqmbqVVKfts22rgS46fAhhxEZeKnfio2Zhgp`,
`BRSkJtCyHESoT5i4EjDr6Yyt6c8F7zaRSTQYt41kLjgg`, `DiMEQj7RTHq9GNu9ENFPDLa14i2f3TQuETCfYB1SRdKa`, `EziVqn7AfudBCYjF7hqSLpi9ERTWFynHQb4YCSHrb7gW`,
`GkR7sLwP6R3AvwpPAqz9JXrvbhVh3M7V1fPf5adLDtvr`, `HHRF63YdM8XT1BctiQD43j9iSAG13Ehbc7EBJapMD4PM`.

## 3. Koin besar DexScreener lain (dev & funder dari log)
| Koin | Naik dari lulus (ATH) | mcap | Dev | Funder |
|---|---|---|---|---|
| DESSA | +163.809% | $4,35 jt | CWpPwPQq | exchange (KNOWN_LIST) |
| Kimchi | +99.233% | $1,99 jt | EFSr396A | tak ketemu |
| BLOB | +76.413% | $3,27 jt | 9iSKstsy | exchange |
| mihao | +70.699% | $8,65 jt | 9tH2zRYd | tak ketemu |
| CLAUDIA | +27.344% | $6,09 jt | bwamJzzt (9 koin DS) | tak ketemu |
| ANTS | +24.511% | $6,62 jt | 49TMLwXU | exchange BmFdpraQ |
| USOI | +17.580% | $5,12 jt | 8QGjHU7k | wallet ramai |
| TTWO | +13.617% | $3,67 jt | 5Np21Upn | wallet ramai |
| GODF (is6MTRHE) | +11.892% | $3,55 jt | HZZxZkYT | exchange |
| GTAVI | +8.711% | $2,50 jt | Ev7CJg57 | bot hub P2ttSJ25 |
| JALU | +2.781% | $798 rb | 2oi1WmQy | bot hub |

## 4. Angka keseluruhan
- Lulus Pump.fun 1 Okt–18 Nov: 44.057 koin, 33.275 dev. DexScreener: 5.992 koin, 4.673 dev.
- 819 koin hidup (769 dev, 125 wallet sekali pakai); 152 koin ≥ $100k; 66 ≥ $1 jt.
- Family: 97 asal jelas dari 443 dev (batch 1).
