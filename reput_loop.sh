#!/usr/bin/env bash
# every 10 min for 8 h: family view (famview) -> creator + entity screen -> watchlist (data/v3/reput/watchlist.json)
cd /root/sol
for i in $(seq 1 48); do
  ./target/release/jaringan famview data/v3/fam2 data/v3/dev data/v3/view > /tmp/fv.json 2>&1
  echo "$(date -u +%H:%M) $(./target/release/jaringan reput data/v3/universe data/v3/dev data/v3/rise data/v3/view/families_active.jsonl data/v3/ath data/v3/reput 2>&1)" >> data/v3/reput/run.log
  sleep 600
done
