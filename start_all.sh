#!/usr/bin/env bash
# 21 Nov 22:55 UTC: all Helius jobs sized to the new key's measured limit (~15 req/s; 45 req/s gave 87% HTTP 429).
# Stop: bash stop_helius.sh
cd /root/sol
export JAR_SCOPE_FILE=/root/sol/data/v3/reput/ds_creators.txt
for i in 0 1 2; do
  JAR_SHARD=$i/3 JAR_RPS=1 nohup ./target/release/jaringan fam2 data/v3/dev data/v3/fam2 1 >> data/v3/fam2/run_$i.log 2>&1 < /dev/null &
done
JAR_RPS=1 nohup ./target/release/jaringan coinrise ALLDEVS data/v3/dev data/v3/rise 1 >> data/v3/rise/run.log 2>&1 < /dev/null &
for i in 0 1; do
  JAR_SHARD=$i/2 JAR_RPS=2 nohup ./target/release/jaringan devtrace data/v3/universe data/v3/dev 1 >> data/v3/dev/run_$i.log 2>&1 < /dev/null &
done
for i in 0 1 2 3; do
  JAR_SHARD=$i/4 JAR_RPS=2 nohup ./target/release/jaringan athgt data/v3/universe data/v3/dev data/v3/ath >> data/v3/ath/run_$i.log 2>&1 < /dev/null &
done
