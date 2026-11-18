#!/bin/bash
# Every 30 min: rebuild everything shown on the web page from ALL data downloaded so far (pulls keep running separately).
cd /root/jaringan
while true; do
  L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
  T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl data/p2/tr_2026-*.part_*.jsonl data/p2/tr_emx.jsonl 2>/dev/null | paste -sd,)
  F=$(ls data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | paste -sd,)
  ./target/release/jaringan outcomes data/p4/outcomes_all.csv $L $T > /dev/null 2>&1
  ./target/release/jaringan team $L $T $F data/p4/outcomes_all.csv j1 0.0146 > /dev/null 2>&1
  ./target/release/jaringan score $L $T $F data/p4/outcomes_all.csv > logs/score_last.log 2>&1
  ./target/release/jaringan cats $L $T data/ml/cats.tsv > /dev/null 2>&1
  ./target/release/jaringan gradsim $L $T data/p2amm/amm2.jsonl 0.0146 71.08 > /dev/null 2>&1
  echo "$(date -u +%FT%TZ) $(cut -c1-220 logs/score_last.log)" >> reports/refresh_log.md
  sleep 1800
done
