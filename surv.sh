#!/bin/bash
# 110 DexScreener survivors (1 Oct - 18 Nov): exact roles (+curve), dev profile, curve trades, pool candles, performance.
cd /root/jaringan || exit 1
D=data/roots; G=$D/gt; mkdir -p $G
./target/release/jaringan roles $D/surv_mints.txt $D/roles2.jsonl | tail -n 1
jq -r 'select(.creator)|[.creator,.bt,.mint]|@tsv' $D/roles2.jsonl > $D/devprof_in.tsv
./target/release/jaringan devprof $D/devprof_in.tsv $D/devprof.jsonl | tail -n 1
# mint curve bt slot pool gbt gslot
jq -r 'select(.curve)|[.mint,.curve,.bt,.slot]|@tsv' $D/roles2.jsonl | sort -u -k1,1 > $D/s_create.tsv
cat data/mig_oct.jsonl data/raw/migrations.jsonl data/mig_new.jsonl | jq -r '[.mint,.pool,.bt,.slot]|@tsv' | sort -u -k1,1 > $D/mig_all.tsv
gawk -F'\t' 'FNR==NR{p[$1]=$2"\t"$3"\t"$4;next} ($1 in p){print $0"\t"p[$1]}' $D/mig_all.tsv $D/s_create.tsv > $D/s.tsv
awk -F'\t' '{print $1","$2","$3}' $D/s.tsv > $D/s_list.csv
./target/release/jaringan trades $D/s_list.csv $D/s_trades.jsonl > $D/s_trades.log 2>&1; tail -n 1 $D/s_trades.log
gt() { # url out : retry until the response holds candles
  for t in 1 2 3 4 5; do
    curl -s -m 60 "$1" > "$2.tmp"
    if jq -e '.data.attributes.ohlcv_list' "$2.tmp" >/dev/null 2>&1; then mv "$2.tmp" "$2"; sleep 2.5; return; fi
    sleep 15
  done
  rm -f "$2.tmp"
}
while IFS=$'\t' read -r mint curve bt slot pool gbt gslot; do
  [ -s $G/$mint.min.json ] || gt "https://api.geckoterminal.com/api/v2/networks/solana/pools/$pool/ohlcv/minute?aggregate=1&limit=1000&currency=token&before_timestamp=$((gbt+60000))" $G/$mint.min.json
  [ -s $G/$mint.hour.json ] || gt "https://api.geckoterminal.com/api/v2/networks/solana/pools/$pool/ohlcv/hour?aggregate=1&limit=1000&currency=token" $G/$mint.hour.json
done < $D/s.tsv
for f in $G/*.min.json; do m=$(basename $f .min.json); jq -r --arg m $m '.data.attributes.ohlcv_list[]? | [$m, .[0], .[2], .[4]] | @tsv' $f; done > $D/s_min.tsv
for f in $G/*.hour.json; do m=$(basename $f .hour.json); jq -r --arg m $m '.data.attributes.ohlcv_list[]? | [$m, .[0], .[2], .[4]] | @tsv' $f; done > $D/s_hour.tsv
jq -r '.mint as $m | .t[]? | [$m, .[0], .[3], .[8], .[9]] | @tsv' $D/s_trades.jsonl > $D/s_curve.tsv
jq -r '[.mint,.sym,.mcap]|@tsv' $D/surv.jsonl > $D/s_sym.tsv
gawk -f m95.awk $D/s.tsv $D/s_curve.tsv $D/s_min.tsv $D/s_hour.tsv $D/s_sym.tsv > $D/s_perf.tsv 2> $D/s_perf.err
echo "candle_files=$(ls $G | wc -l) perf_rows=$(($(wc -l < $D/s_perf.tsv)-1))"
echo SURV_DONE
