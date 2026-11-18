#!/bin/bash
# Moonshots 6-18 Nov (Pump.fun, graduated, NOW mcap >= $100k with >= $20k liquidity = survived) ->
# creator -> funder chain 4 levels (Helius, full history) -> every wallet funded by those funders (crawl, outgoing SOL)
# -> which other creators/devs are connected, and did they also make moonshots.
cd /root/jaringan || exit 1
D=data/moonall
until [ -f $D/ds.done ]; do sleep 20; done
jq -c "select((.mcap//0)>=100000 and (.liq//0)>=20000)" $D/ds.jsonl > $D/moon.jsonl
jq -r .mint $D/moon.jsonl > $D/moon_mints.txt
# creator of every create 6-18 Nov
jq -r "select(.kind==\"create\")|[.mint,.creator,.user,.bt,.slot]|@tsv" data/raw/launches_*.jsonl data/launches_2026-11-1*.jsonl > $D/all_creates.tsv
gawk -F"\t" "FNR==NR{m[\$1]=1;next} (\$1 in m)" $D/moon_mints.txt $D/all_creates.tsv | sort -u -k1,1 > $D/moon_creates.tsv
cut -f2 $D/moon_creates.tsv | sort -u > $D/lvl1.txt
echo "moon=$(wc -l < $D/moon.jsonl) with_creator=$(wc -l < $D/moon_creates.tsv) creators=$(wc -l < $D/lvl1.txt)"
for L in 1 2 3 4; do
  ./target/release/jaringan fund $D/lvl$L.txt $D/fund_l$L.jsonl > /dev/null 2>&1
  jq -r "select(.funder!=null)|.funder" $D/fund_l$L.jsonl | sort -u | grep -vxF -f data/p3/exchange_list.txt > $D/lvl$((L+1)).txt
  echo "level $L funders=$(wc -l < $D/lvl$((L+1)).txt)"
done
cat $D/lvl2.txt $D/lvl3.txt $D/lvl4.txt $D/lvl5.txt | sort -u > $D/chain_funders.txt
./target/release/jaringan crawl $D/chain_funders.txt 1 200000 2 $(date -u -d 2026-11-01 +%s) $D/crawl.jsonl > $D/crawl.log 2>&1
echo "crawl_edges=$(wc -l < $D/crawl.jsonl)"
cat checkpoints/credits/*.txt | tr "\n" " "; echo
echo MOONALL_DONE
