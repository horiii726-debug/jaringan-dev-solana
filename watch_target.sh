#!/usr/bin/env bash
# rebuild typed family view + quality every 10 min; stop when >= 300 active families and >= 3000 wallets, or after 3 h
cd /root/sol
for i in $(seq 1 18); do
  ./target/release/jaringan famview data/v3/fam2 data/v3/dev data/v3/view > /tmp/fv.json 2>&1
  ./target/release/jaringan qfam data/v3/dev data/v3/rise data/v3/fam2/families.jsonl data/v3/quality/fam2 > /tmp/qf.json 2>&1
  f=$(jq .families_active data/v3/view/summary.json); w=$(jq .wallets_in_active_families data/v3/view/summary.json)
  echo "$(date -u +%H:%M) families=$f wallets=$w quality=$(jq .quality /tmp/qf.json)" >> data/v3/view/progress.log
  if [ "$f" -ge 300 ] && [ "$w" -ge 3000 ]; then echo TARGET; exit 0; fi
  sleep 600
done
echo TIMEOUT
