#!/bin/bash
# Parallel DexScreener pull for graduated mints not yet in ds.jsonl (6 workers, 30 tokens per call).
cd /root/jaringan || exit 1
D=data/moonall
jq -r .mint $D/ds.jsonl | sort -u > $D/dsdone.txt
grep -vxF -f $D/dsdone.txt $D/grad_mints.txt > $D/dstodo.txt
rm -f $D/g_* ; rm -rf $D/parts ; mkdir -p $D/parts
split -l 30 $D/dstodo.txt $D/parts/p_
echo "todo=$(wc -l < $D/dstodo.txt)"
one() {
  f=$1; L=$(paste -sd, "$f")
  for t in 1 2 3; do
    curl -s -m 40 "https://api.dexscreener.com/tokens/v1/solana/$L" > "$f.json" && jq -e 'type=="array"' "$f.json" >/dev/null 2>&1 && break
    sleep 3
  done
  jq -c 'group_by(.baseToken.address)[] | max_by(.liquidity.usd // 0) | {mint: .baseToken.address, sym: .baseToken.symbol, dex: .dexId, mcap: (.marketCap // .fdv), liq: .liquidity.usd, vol24: .volume.h24, pair_created: .pairCreatedAt}' "$f.json" > "$f.out" 2>/dev/null
}
export -f one
ls $D/parts/p_* | xargs -P 6 -I{} bash -c 'one {}'
cat $D/parts/*.out >> $D/ds.jsonl
echo "rows=$(wc -l < $D/ds.jsonl)"
touch $D/ds.done
