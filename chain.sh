#!/bin/bash
# Walk funding chains upward (dev -> funder -> funder of funder ...) up to 5 levels for a wallet list.
# Stops at wallets with deep history (>=3000 tx) or busy activity (service/exchange). Usage: chain.sh <wallets> <tag>
cd /root/jaringan
W=$1; TAG=$2
cp $W /tmp/chain_cur.txt
for lvl in 1 2 3 4 5; do
  cat data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null > /tmp/allfund.jsonl
  jq -r .w /tmp/allfund.jsonl | sort -u > /tmp/fdone.txt
  comm -23 <(sort -u /tmp/chain_cur.txt) /tmp/fdone.txt > /tmp/chain_todo.txt
  [ -s /tmp/chain_todo.txt ] && ./fund_par.sh /tmp/chain_todo.txt data/p4/chain_${TAG}_l$lvl 4 > /dev/null 2>&1
  cat data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null > /tmp/allfund.jsonl
  # next level: funders of the current level that are not deep
  jq -r "select(.deep!=true and .funder!=null)|[.w,.funder]|@tsv" /tmp/allfund.jsonl | gawk -F"\t" "ARGIND==1{c[\$1]=1;next} (\$1 in c){print \$2}" /tmp/chain_cur.txt - | sort -u > /tmp/chain_next.txt
  echo "level $lvl: wallets $(wc -l < /tmp/chain_cur.txt) -> funders $(wc -l < /tmp/chain_next.txt)"
  [ -s /tmp/chain_next.txt ] || break
  cp /tmp/chain_next.txt /tmp/chain_cur.txt
done
echo CHAIN_DONE
