#!/bin/sh
# usage: fund_par.sh <wallet_list> <out_prefix> [shards]  -> runs shards in parallel, merges into <out_prefix>.jsonl
cd /root/jaringan
L=$1; P=$2; K=${3:-16}
rm -f $P.part_*.txt
split -n l/$K -d -a 2 $L $P.part_ --additional-suffix=.txt
for f in $P.part_*.txt; do
  s=$(basename $f .txt)
  JAR_RPS=1 ./target/release/jaringan fund $f $(dirname $P)/$s.jsonl > logs/$s.log 2>&1 &
done
wait
cat $(dirname $P)/$(basename $P).part_*.jsonl > $P.jsonl
grep -h ERROR logs/$(basename $P).part_*.log | head -3
echo FUND_DONE $(wc -l < $P.jsonl)
