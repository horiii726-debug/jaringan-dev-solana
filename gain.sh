#!/bin/bash
# 18 Nov: hourly PumpSwap candles (GeckoTerminal, free, price in SOL) for the 819 alive DexScreener coins
cd /root/jaringan || exit 1
R=data/roots; G=$R/gt; mkdir -p $G
gawk -F"\t" "FNR==NR{p[\$1]=\$2;next} (\$1 in p){print \$1\"\t\"p[\$1]}" $R/grad_pools.tsv $R/alive_mints.txt > $R/alive_pools.tsv
echo "pools=$(wc -l < $R/alive_pools.tsv)"
while IFS=$'\t' read -r mint pool; do
  f=$G/$mint.hour.json
  [ -s $f ] && jq -e ".data" $f >/dev/null 2>&1 && continue
  for t in 1 2 3; do curl -s -m 60 "https://api.geckoterminal.com/api/v2/networks/solana/pools/$pool/ohlcv/hour?aggregate=1&limit=1000&currency=token" > $f; jq -e ".data" $f >/dev/null 2>&1 && break; sleep 20; done
  sleep 2.2
done < $R/alive_pools.tsv
echo GAIN_DONE
