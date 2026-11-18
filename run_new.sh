#!/bin/bash
# 18 Nov: new Helius key -> launches 16 Nov..now, migrations, AMM pool trades (3 pages) of graduated SOL coins.
cd /root/jaringan || exit 1
NOW=$(date -u +%s)
for d in 2026-11-16 2026-11-17 2026-11-18; do
  S=$(date -u -d "$d" +%s); E=$((S+86400)); [ $E -gt $NOW ] && E=$NOW
  ./target/release/jaringan launches $S $E data/launches_$d.jsonl > logs/launches_$d.log 2>&1 &
done
S=$(date -u -d 2026-11-16 +%s)
./target/release/jaringan migrations $S $NOW data/mig_new.jsonl > logs/mig_new.log 2>&1
wait
jq -r "select(.quote==\"So11111111111111111111111111111111111111112\")|[.mint, (\"pool=\"+.pool), .slot, .bt]|@csv" data/mig_new.jsonl | tr -d "\"" | sort -u > data/mig_new_list.txt
JAR_AMM_MAX_PAGES=3 ./target/release/jaringan amm data/mig_new_list.txt data/amm_new.jsonl > logs/amm_new.log 2>&1
cat checkpoints/credits/*.txt | tr "\n" " "; echo
echo NEW_DONE
