#!/bin/bash
# Auto-update loop: each new UTC day -> pull creates, trace funders (chain), pull trades, rebuild teams/score/web.
# Same rules as the backtest (as-of), so new data is filtered and selected the same way.
cd /root/jaringan
while true; do
  D=$(date -u -d "yesterday" +%F)
  if ! grep -q "\"done\":true" data/launches_$D.jsonl.ckpt 2>/dev/null; then
    S=$(date -u -d "$D" +%s); ./target/release/jaringan launches $S $((S+86400)) data/launches_$D.jsonl > logs/launches_$D.log 2>&1
    jq -r "select(.kind==\"create\")|.creator,.user" data/launches_$D.jsonl | sort -u > data/p4/new_devs_$D.txt
    ./chain.sh data/p4/new_devs_$D.txt d$D > logs/chain_$D.log 2>&1
    jq -r "select(.kind==\"create\" and .mayhem==false and (.quote|startswith(\"1111\")))|[.mint,.curve,.bt]|@csv" data/launches_$D.jsonl | tr -d "\"" > data/p2/elig_$D.txt
    ./trades_par.sh data/p2/elig_$D.txt data/p2/tr_$D 4 > logs/trades_$D.log 2>&1
  fi
  L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
  T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl data/p2/tr_2026-*.part_*.jsonl 2>/dev/null | paste -sd,)
  F=$(ls data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | paste -sd,)
  ./target/release/jaringan outcomes data/p4/outcomes_all.csv $L $T > /dev/null 2>&1
  ./target/release/jaringan team $L $T $F data/p4/outcomes_all.csv j1 0.0146 > /dev/null 2>&1
  ./target/release/jaringan score $L $T $F data/p4/outcomes_all.csv > logs/score_last.log 2>&1
  ./target/release/jaringan cats $L $T data/ml/cats.tsv > /dev/null 2>&1
  echo "$(date -u +%FT%TZ) refreshed $(cat logs/score_last.log | cut -c1-200)" >> reports/daily_log.md
  sleep 3600
done
