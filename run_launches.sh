#!/bin/sh
# P1.1: pull every Pump.fun create, one file per UTC day, days in parallel, resumable.
cd /root/jaringan
for d in 2026-11-10 2026-11-11 2026-11-12 2026-11-13 2026-11-14 2026-11-15; do
  S=$(date -u -d "$d" +%s); E=$((S+86400))
  ./target/release/jaringan launches $S $E data/launches_$d.jsonl > logs/launches_$d.log 2>&1 &
done
wait
grep -l ERROR logs/launches_*.log
echo LAUNCHES_DONE
