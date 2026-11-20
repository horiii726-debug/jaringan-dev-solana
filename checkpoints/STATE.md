== 21 Nov 22:55 UTC: batas laju key Helius baru ~15 req/s (45 req/s -> 87% 429). start_all.sh kini: fam2 3x1, coinrise 1, devtrace 2x2, athgt 4x2. ShredStream trial d722f208 (tujuan 62.107.25.205:8001) menunggu aktivasi manual via Discord; vast3 TANPA UDP -> udpprobe 24 jam di port 8001, log data/shred/probe.log ==
== 21 Nov 22:00 UTC: KEY HELIUS BARU dipasang (.env; key lama di .env.bak_oldkey_21nov). Batas 3M kredit (budget.toml), ledger lama di checkpoints/credits_oldkey_21nov. Semua job: bash start_all.sh (fam2 x3 + coinrise dgn JAR_SCOPE_FILE=data/v3/reput/ds_creators.txt, devtrace v7, athgt x8, reput_loop). Stop: bash stop_helius.sh ==
== 21 Nov 21:35 UTC — HELIUS HABIS ("max usage reached", HTTP 429) — SEMUA JOB HELIUS DIHENTIKAN ==
Rencana prediksi+eksekusi: checkpoints/PLAN_V3_PREDIKSI.md (pemilik 21 Nov). Watchlist: data/v3/reput/watchlist.json (reput_loop.sh tiap 10 mnt).
Pemilik: hanya creator yg koinnya ada di DexScreener -> 3.611 creator (5.023 non-DS dibuang). devtrace v7 (ds_creators_only=1, graduated_all=0, fallback 0), athgt scope DS, reput only_dexscreener=1 (arg baru: reput <universe> <dev> <rise> <fam> <ath_dir> <out>).
ATH: formula 2 (quote+virtual_quote)/base x base_supply, tervalidasi vs GeckoTerminal +-5% (data/v3/ath_val). Data formula 1 diarsip data/v3/ath/old_formula1. ath_90.jsonl = baris validasi.
Saringan (config/reput.toml v3): rug>40%, sepi>=50% (pool<200tx/<50 trader/kurva<50 trade), palsu (jual<5%), jual<=10s pada >=50% koin. Saat berhenti: ELIMINASI 297, LOLOS_BELUM_TERBUKTI 252, LOLOS_DATA_KURANG 11, BELUM_CUKUP_DATA 3.051.
Bersih-bersih kuota: roles -200 baris QUOTA, devs -9 baris pages=0, rise/coins -225 baris pages=0 (backup *.bak_quota). devtrace+coinrise kini return Err pada QUOTA.
Lanjut (butuh kredit Helius): bash restart_v7.sh (devtrace + athgt x8 + reput_loop), lalu fam2 x3 + coinrise manual. Masih jalan: dexs, mosq.

== RESUME 21 Nov 04:10 WIB (21:10 UTC) — REPUTASI CREATOR, mulai dari sini ==
Pemilik: creator = dev. Per creator: jumlah koin, BERHASIL (ATH >= $50k) vs GAGAL, koin mana yg berhasil, naik min/max % dari harga launch, ATH max, cara exit, riwayat per koin.
BARU (belum commit): src/reput.rs (jaringan reput <dev> <rise> <families.jsonl> <ath_dir> <out>), src/athgt.rs (ATH dari trade pool PumpSwap via Helius; JAR_SHARD i/8, JAR_ATH_PAGES=20), devtrace v6 (orphan_batch: 7,8k creator roles yg belum ditelusuri), amm.rs helper pub.
JALAN: athgt x8 (start_ath.sh; log data/v3/ath/run_i.log, hasil ath_i.jsonl) | reput_loop.sh tiap 10 mnt -> data/v3/reput/dev_reputation.tsv + creator_coins.tsv, ringkas di data/v3/reput/run.log
Temuan: SOL $67,5 -> kurva lulus ~410 SOL = ~$28k; pool mulai ~325 SOL; $50k hanya bisa di DEX. GeckoTerminal publik ~1 call/mnt (429) -> pakai Helius. Pump.fun API diblok Cloudflare.
Temuan: 104 creator lengkap tapi 0 koin = semua pembayar beda (creator cuma alamat fee) -> kolom pembayar_lain. Pool aneh (mulai 1-3 SOL, liq $3) dibuang dari ATH.
Batas: page cap 20 -> koin ramai ATH = batas bawah (ATH_lengkap=batas_bawah); jual dev lewat wallet lain tdk terlihat.
Restart aman: bash start_ath.sh (JANGAN pkill dari baris ssh yg memuat pola yg sama).

== RESUME 21 Nov 03:20 WIB (20:20 UTC) — mulai dari sini ==
JALAN (nohup, Rust): dexs (koin 30 hr, poll 5 mnt) | devtrace (JAR_RPS=3; graduated_all=1 batch 200) | coinrise ALLDEVS | fam2 x3 shard (JAR_SHARD i/3, JAR_RPS=2; up=Helius getTransfersByAddress, down=CoinStats->Helius, kelas=Tatum+RPC publik) | mosq (MadeOnSol leaderboard+recent-bonds, tunggu reset 00:00 UTC) | watch_target.sh (stop >=300 family & >=3000 wallet atau 3 jam; log data/v3/view/progress.log)
Restart pola: pkill -f "release/jaringan NAM[A]" (jangan pkill pola yg cocok dgn shell sendiri)
Hasil: data/v3/view (families_active, independent_devs, one_time_wallets), data/v3/quality/fam2/quality_families.tsv (5 BERKUALITAS: EVkj48m7, BRSkJtCy, Ed6Xaynq, 3HUQhVLf, 8BYUixL8), data/v3/rise/coin_ath.tsv, rise/blacklist*.txt, data/v3/unified (lama+baru bersih), data/legacy_clean
Aturan pemilik: aktif <=30 hari; jual<=60s boleh asal bukan dump>=90% (maks 25% koin); rug<=40% dihitung hanya jika dev jual <=1 jam; dev rug cepat -> seluruh family blacklist; independen & sekali pakai dipisah
Kunci: .env HELIUS, .madeonsol.env (BASIC 200/hr), .tatum.env (RPC saja; Data API tdk dukung Solana), .coinstats.env (hanya tx keluar). Helius Wallet API funded-by/identity = 403 (paket Free).
Artifact: peta https://claude.ai/artifact/S9hAJtTsWKxkx8M6AUhYHF ; tabel koin https://claude.ai/artifact/84nMgXFkpU3a8fB1TCpfpa
Berikutnya: setelah watch_target selesai -> famview+qfam, perbarui graph.json & tabel koin, kirim ringkasan; kode belum di-commit (pemilik belum minta)

== V3 FAMILY (20 Nov malam) ==
RUNNING: dexs (30 hari, v3 disk-hemat) + devtrace (v2, fallback >=$10k) + famv3 (loop 5 mnt) -> data/v3/family/summary.json, families.jsonl
target pemilik: 1000 family aktif; kredit boleh semua (budget 20M, stop nyata = kuota Helius)
cek: cat data/v3/family/summary.json; tail -1 data/v3/universe/runs.jsonl; tail -1 data/v3/dev/run.log

== V3 LIVE (20 Nov sore) ==
RUNNING (nohup): jaringan dexs data/v3/universe 0 (poll 5 mnt; log runs.jsonl) + jaringan devtrace data/v3/universe data/v3/dev 1 (devs.jsonl: baris terakhir per dev = sah)
config: dexscreener.toml v1, tiers.toml, devtrace.toml v1 (PROVISIONAL)
hasil awal: 116 koin lolos, 113 dev: 69 1-koin (lengkap), 26 terpotong, POLA_RUG 5, CAMPUR 11, BAGUS 1, LEMAH 1; kredit ~21,6k
next: Stage 3/4 family (kelas wallet + bukti E1-E7), CRITIC di akhir

== V3 (20 Nov) — mulai dari sini ==
phase: V3 Stage 0 audit   updated: 2026-11-20
done: T0 r1 (CRITIC 3.0 FAIL), r2 fixes
next: CRITIC r2 of T0; then owner OK for Stage 1 (DexScreener universe, Rust)
blocked on owner: Stage 1 OK; Helius credit cap; permission for .claude/agents/critic.md
budget: Helius new key, ~1 credit used; MadeOnSol 3 of 200/day
data: data/ extracted; legacy family_a/family_export/labels.tsv = LEGACY_PRE_V3, not evidence
owner scope: trace devs of coins reaching >= $50k tier (higher better) + random control sample
last result: reports/v3/IMPLEMENTATION_AUDIT.md

== UPDATE 18 Nov (setelah chat di-clear, mulai dari sini) ==
- tpsim sudah diperbaiki (commit fe6def2): creator "sering lulus" -> TEST 13-18 Nov TP10 +27,6%/trade (ikut dev jual), +49,7% (tanpa). Lihat STEP_LOG baris terakhir.
- JOB BERJALAN (nohup): holders_all.sh = cluster pemegang ala Bubblemaps untuk JALU, GATO, lalu 110 koin pemenang.
  Cek: tail data/holders/holders_all.log ; hasil: data/holders/SUMMARY.tsv ; per koin: data/holders/<mint>/clusters.tsv
  (kolom: funder, label, jumlah holder, % supply; "OPERATOR" = keluarga dev, exchange/bot tidak dihitung). Berhenti otomatis di batas kredit 300k.
- Berikutnya: (1) baca SUMMARY.tsv -> koin dgn keluarga dev besar + siapa funder-nya; cek apakah funder itu juga di balik koin lain
  (gabung dgn data/roots/*fund*.jsonl); (2) uji minggu segar (19 Nov+) untuk strategi creator sering lulus; (3) paper monitor.

phase: DEX-FIRST RESEARCH (18 Nov)   updated: 2026-11-18   STATUS: jobs may still run (devhist_alive.sh -> devrep.sh, synd.sh, dexfirst.sh)
RESUME (baca ini dulu):
- Kunci di .env (Helius 2 kunci, Shyft, Bitquery). Batas kredit Helius 300k (config/budget.toml), terpakai ~140k.
- Data: data/roots/ (44.057 koin Pump.fun lulus 1 Okt-18 Nov; ds.jsonl DexScreener 5.992; 819 hidup; 110 >=100k),
  data/grad/ (uji creator sering lulus + TP sim + Kelly), data/fwd/ (Bitquery 17-18 Nov), data/moonall/.
- Hasil inti:
  * 359.705 koin 6-18 Nov: lulus normal 1,57%. Creator dipilih 6-12 Nov (>=3 koin, >=30% lulus): koin barunya 13-18 Nov lulus 8,5% (5x).
  * TP sim (beli slot+2, keluar TP/dev jual/30 mnt): TEST 13-18 Nov rugi (-0,6..-4,1%/trade); Kelly 1/4 $100 -> $52-$69.
    Koin lulus n=37 rata +65% (keluar dini krn dev jual 77% trade); koin gagal n=408 rata -7%.
  * Sindikat (aturan ketat, penghubung bukan exchange/bot/wallet ramai): 2 kecil; 1.268 dev independen. Versi longgar 11 sindikat/143 dev
    lewat wallet ramai (5tzFkiKs 38 dev dana pertama; 21wG4F3Z 17 dev penerima) = BELUM TERBUKTI, perlu bedah manual.
- Berikutnya: (1) varian exit tanpa "ikut dev jual" (tahan sampai TP/lulus/stop), pilih di TRAIN lalu uji SEKALI di TEST;
  (2) bedah 5tzFkiKs, 21wG4F3Z, 8a683UoG, 5t4Tz7qe, pwZpq97v; (3) baca data/roots/devrep.txt (riwayat gagal vs DEX per dev);
  (4) data/dexfirst.log (Meteora/LaunchLab dll).
- Aturan pemilik: mulai dari koin DexScreener (jangan sapu semua koin Pump.fun); transfer acak/kecil BUKAN keluarga; exchange bukan founder.

phase: REVIEW FIX (18 Nov)   updated: 2026-11-18   STATUS: F1 done, F2 code done (paper run needs key), F3/F4 honest.rs done, F6 done, F5/F8 wait for keys
honest result: NO EDGE with reachable entry (see reports/honest/, STEP_LOG 18 Nov)
blocked: .env (password of secrets/env.enc or new keys after rotation); repo public -> owner

> **INVALID (18 Nov review)** — semua hasil 0o / j1–j5 / "Jito-first", gradsim r1h/r6h/r24h, simulasi Kelly/posisi ($14 → $621.869), model $50k walk-forward (wf.txt, AUC 0,94–0,99), tes "forward" tim, dan kelas A/B/C sebagai daftar beli **TIDAK SAH**. Sebab: (A) harga masuk tak terjangkau (98% koin label lulus di slot create; deteksi kita p50 2,6 dtk), (B) fitur funding bocor label (has_fund saja AUC 0,93–0,99), (C) gradsim tak lihat crash & data AMM ~26 dtk bukan 1 jam. Angka jujur: reports/honest/.

phase: P7 paper   updated: 2026-11-17   STATUS: SEMUA DIHENTIKAN atas perintah pemilik
done: P0-P6, full-history reputation, origin (Shyft), links (Bitquery), pool check, paper executor, GitHub backup
next (saat lanjut): uji 4 aturan jual pelindung (lulus / trailing 30% / 10 menit / gabungan); putuskan kelas A (pompa-lalu-mati)
blocked on owner: pilihan aturan ketat vs kelas PUMP; live tetap butuh kata tegas pemilik
budget: Helius ~0.77M of 1M; Shyft/Bitquery dipakai
data: data/ (VPS), backup GitHub horiii726-debug/jaringan-dev-solana (main)
resume: tmux new -d -s shadow "JAR_EXEC_MODE=paper JAR_RPC_URL=https://rpc.shyft.to?api_key=$SHYFT_KEY ./target/release/jaringan shadow reports/web/watch_all.json reports/shadow"
        tmux new -d -s daily2 ./daily2.sh ; tmux new -d -s web "./target/release/jaringan serve 8899"
