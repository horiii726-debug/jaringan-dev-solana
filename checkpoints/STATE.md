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
