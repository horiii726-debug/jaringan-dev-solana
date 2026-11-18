#!/bin/bash
cd /root/jaringan
for f in data/hist/part_*.txt; do s=$(basename $f .txt); JAR_RPS=2 ./target/release/jaringan history $f 5 data/hist/h_$s.jsonl > logs/hist_$s.log 2>&1 & done
wait
cat data/hist/h_part_*.jsonl > data/hist/history.jsonl
echo HIST_DONE
