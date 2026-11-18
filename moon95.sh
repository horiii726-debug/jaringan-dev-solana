#!/bin/bash
# Interim (before Stage 1 DB): for the surviving moonshots in data/moonall/moon.jsonl
#  1) curve trades after create (Helius, jaringan trades) -> reachable entry price (first trade after the create slot)
#  2) PumpSwap pool candles (GeckoTerminal public API, price in SOL): minute candles for the first 1000 min after
#     graduation, hourly candles for the long run -> ATH, time to ATH, minutes held above 2x/5x/10x entry, now.
cd /root/jaringan || exit 1
D=data/moonall; G=$D/gt; mkdir -p $G
# mint, curve, bt, slot, pool, gbt
jq -r 'select(.kind=="create")|[.mint,.curve,.bt,.slot]|@tsv' data/raw/launches_*.jsonl data/launches_2026-11-1*.jsonl \
  | gawk -F'\t' 'FNR==NR{m[$1]=1;next} ($1 in m)&&!s[$1]++' $D/moon_mints.txt - > $D/m95_create.tsv
jq -r '[.mint,.pool,.bt,.slot]|@tsv' data/raw/migrations.jsonl data/mig_new.jsonl | sort -u -k1,1 > $D/mig_all.tsv
gawk -F'\t' 'FNR==NR{p[$1]=$2"\t"$3"\t"$4;next} ($1 in p){print $0"\t"p[$1]}' $D/mig_all.tsv $D/m95_create.tsv > $D/m95.tsv
echo "coins=$(wc -l < $D/m95.tsv)"
# 1) curve trades
awk -F'\t' '{print $1","$2","$3}' $D/m95.tsv > $D/m95_list.csv
./target/release/jaringan trades $D/m95_list.csv $D/m95_trades.jsonl > $D/m95_trades.log 2>&1
tail -n 1 $D/m95_trades.log
# 2) candles
while IFS=$'\t' read -r mint curve bt slot pool gbt gslot; do
  [ -s $G/$mint.min.json ] || { curl -s -m 60 "https://api.geckoterminal.com/api/v2/networks/solana/pools/$pool/ohlcv/minute?aggregate=1&limit=1000&currency=token&before_timestamp=$((gbt+60000))" > $G/$mint.min.json; sleep 2.2; }
  [ -s $G/$mint.hour.json ] || { curl -s -m 60 "https://api.geckoterminal.com/api/v2/networks/solana/pools/$pool/ohlcv/hour?aggregate=1&limit=1000&currency=token" > $G/$mint.hour.json; sleep 2.2; }
done < $D/m95.tsv
echo "candles=$(ls $G | wc -l)"
echo M95_DONE
