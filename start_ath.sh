#!/usr/bin/env bash
# restart athgt (4 Helius shards) and the creator-report loop
cd /root/sol
pkill -f "release/jaringan athgt"
pkill -f "bash ./reput_loop.sh"
sleep 1
for i in 0 1 2 3 4 5 6 7; do
  JAR_SHARD=$i/8 JAR_RPS=3 nohup ./target/release/jaringan athgt data/v3/universe data/v3/dev data/v3/ath >> data/v3/ath/run_$i.log 2>&1 < /dev/null &
done
nohup ./reput_loop.sh > /dev/null 2>&1 < /dev/null &
