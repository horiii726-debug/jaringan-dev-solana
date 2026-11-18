#!/bin/bash
cd /root/jaringan
for f in data/p2amm/rest_*.txt; do s=$(basename $f .txt); JAR_RPS=2 ./target/release/jaringan amm $f data/p2amm/amm_$s.jsonl > logs/amm_$s.log 2>&1 & done
wait
cat data/p2amm/amm_big.jsonl data/p2amm/amm_rest_*.jsonl > data/p2amm/amm_all.jsonl
jq -r "select(.t)|[.mint, ([.t[]|select(.[6]>=20000000000 and .[5]>0)|(.[6]/.[5]*1000000)]|max // 0)]|@tsv" data/p2amm/amm_all.jsonl | awk -F"\t" -v p=71.08 "{printf \"%s\t%.0f\t%.0f\n\", \$1, \$2, \$2*p}" > data/p2amm/peak_mcap.tsv
awk -F"\t" "\$3>=50000" data/p2amm/peak_mcap.tsv > data/p2amm/big50k.tsv
echo "coins=$(wc -l < data/p2amm/peak_mcap.tsv) >=50k=$(wc -l < data/p2amm/big50k.tsv) >=1M=$(awk -F"\t" "\$3>=1000000" data/p2amm/big50k.tsv | wc -l) >=10M=$(awk -F"\t" "\$3>=10000000" data/p2amm/big50k.tsv | wc -l)"
jq -r "select(.kind==\"create\")|[.mint,.creator,.user]|@tsv" data/launches_2026-1*.jsonl | gawk -F"\t" "ARGIND==1{b[\$1]=1;next} (\$1 in b){print \$2; if(\$3!=\$2) print \$3}" data/p2amm/big50k.tsv - | sort -u > data/p4/big_devs.txt
echo "devs=$(wc -l < data/p4/big_devs.txt)"
./chain.sh data/p4/big_devs.txt big > logs/chain_big.log 2>&1
echo BIG_DONE
