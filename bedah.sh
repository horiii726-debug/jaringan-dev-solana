#!/bin/bash
cd /root/jaringan
until grep -q CHAIN_DONE logs/chain_bqbig.log; do sleep 20; done
cat data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | jq -r "[.w, (.funder//\"-\"), ((.deep//false)|tostring)]|@tsv" > /tmp/fx2.tsv
cut -f2 /tmp/allcu.tsv | sort | uniq -c | awk "{print \$2\"\t\"\$1}" > /tmp/ccount.tsv
jq -r "[.mint,.buyer]|@tsv" data/bq/early.jsonl | sort -u | cut -f2 | sort | uniq -c | awk "{print \$2\"\t\"\$1}" > /tmp/rep.tsv
jq -r "[.mint,.buyer]|@tsv" data/bq/early.jsonl | sort -u > /tmp/eb.tsv
sort -t"	" -k2,2nr data/bq/big_bq.tsv > /tmp/bigsorted.tsv
gawk -f bedah.awk data/p3/exchange_list.txt /tmp/fx2.tsv /tmp/ccount.tsv /tmp/rep.tsv /tmp/eb.tsv data/bq/big_creators.tsv /tmp/bigsorted.tsv > reports/bq_bedah.tsv 2> reports/bq_bedah_header.txt
echo BEDAH_DONE
