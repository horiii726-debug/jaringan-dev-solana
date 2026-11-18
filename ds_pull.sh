#!/bin/bash
# current market cap / liquidity of graduated coins (DexScreener public API, 30 tokens per call, <=300 calls/min)
cd /root/jaringan || exit 1
D=data/moonall; : > $D/ds.jsonl
split -l 30 $D/grad_mints.txt $D/g_
for f in $D/g_*; do
  L=$(paste -sd, $f)
  for t in 1 2 3; do
    curl -s -m 30 "https://api.dexscreener.com/tokens/v1/solana/$L" > $D/r.json && jq -e "type==\"array\"" $D/r.json >/dev/null 2>&1 && break; sleep 5
  done
  jq -c "group_by(.baseToken.address)[] | max_by(.liquidity.usd // 0) | {mint: .baseToken.address, sym: .baseToken.symbol, dex: .dexId, mcap: (.marketCap // .fdv), liq: .liquidity.usd, vol24: .volume.h24, pair_created: .pairCreatedAt}" $D/r.json >> $D/ds.jsonl 2>/dev/null
  sleep 0.25
done
rm -f $D/g_*
echo "rows=$(wc -l < $D/ds.jsonl)"; touch $D/ds.done
