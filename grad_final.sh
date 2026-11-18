#!/bin/bash
cd /root/jaringan || exit 1
until grep -q "^DONE" data/grad/trades.log && grep -q "^DONE" data/grad/train_trades.log; do sleep 20; done
L=$(ls data/raw/launches_*.jsonl data/launches_2026-11-1*.jsonl | paste -sd,); M=data/raw/migrations.jsonl,data/mig_new.jsonl
echo "== TRAIN 6-12 Nov (koin creator terpilih, dipakai memilih)"; ./target/release/jaringan tpsim $L data/grad/train_trades.jsonl $M data/grad/sel_creators.txt data/grad/train.json | grep -E "^k2_|coins="
echo "== TEST 13-18 Nov (belum pernah dilihat)"; ./target/release/jaringan tpsim $L data/grad/test_trades.jsonl $M data/grad/sel_creators.txt data/grad/test.json | grep -E "^k2_|coins="
echo "== KELLY 1/4, modal \$100, compounding, maks 1 SOL per trade, SOL \$74.82 (masuk slot +2)"
for s in tp2 tp3 tp5 tp10 ladder_2_3_5_10; do ./target/release/jaringan kelly data/grad/train.json.rows.tsv data/grad/test.json.rows.tsv k2_$s 100 74.82 1; done
echo GRAD_DONE
