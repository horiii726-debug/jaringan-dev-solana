#!/bin/sh
# Backfill the launch universe further back (owner 16 Nov: all data). 2 days in parallel to leave rate for fund/trades.
cd /root/jaringan
d=2026-11-08
while [ "$d" != "2026-10-01" ]; do echo $d; d=$(date -u -d "$d - 1 day" +%F); done | xargs -P 2 -I{} sh -c "S=\$(date -u -d {} +%s); E=\$((S+86400)); ./target/release/jaringan launches \$S \$E data/launches_{}.jsonl > logs/launches_{}.log 2>&1"
echo MORE_DONE
