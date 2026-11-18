#!/bin/bash
# Coins that reached >= $50k market cap (PumpSwap), their devs, funder chains, and the teams behind them.
cd /root/jaringan
true
jq -r "select(.quote==\"So11111111111111111111111111111111111111112\")|[.mint, (\"pool=\"+.pool), .slot, .bt]|@csv" data/p2amm/migrations.jsonl | tr -d "\"" | sort -u > data/p2amm/mig_list.txt
echo "migrations SOL: $(wc -l < data/p2amm/mig_list.txt)"
true
./target/release/jaringan amm data/p2amm/mig_list.txt data/p2amm/amm_big.jsonl
sed -i "s/^amm_max_pages = .*/amm_max_pages = 10/" config/budget.toml
# peak market cap with a liquidity floor of 20 SOL in the pool (thin pools give fake caps)
jq -r "select(.t)|[.mint, ([.t[]|select(.[6]>=20000000000 and .[5]>0)|(.[6]/.[5]*1000000)]|max // 0)]|@tsv" data/p2amm/amm_big.jsonl | awk -F"\t" -v p=71.08 "{printf \"%s\t%.0f\t%.0f\n\", \$1, \$2, \$2*p}" > data/p2amm/peak_mcap.tsv
awk -F"\t" "\$3>=50000" data/p2amm/peak_mcap.tsv > data/p2amm/big50k.tsv
echo "coins >= \$50k: $(wc -l < data/p2amm/big50k.tsv) | >= \$1M: $(awk -F"\t" "\$3>=1000000" data/p2amm/big50k.tsv | wc -l) | >= \$10M: $(awk -F"\t" "\$3>=10000000" data/p2amm/big50k.tsv | wc -l)"
jq -r "select(.kind==\"create\")|[.mint,.creator,.user]|@tsv" data/launches_2026-1*.jsonl | gawk -F"\t" "ARGIND==1{b[\$1]=1;next} (\$1 in b){print \$2; if(\$3!=\$2) print \$3}" data/p2amm/big50k.tsv - | sort -u > data/p4/big_devs.txt
echo "devs of big coins (in our launch files): $(wc -l < data/p4/big_devs.txt)"
./chain.sh data/p4/big_devs.txt big > logs/chain_big.log 2>&1
echo BIG_DONE
