#!/usr/bin/env bash
# 21 Nov: devtrace as 4 parallel shards (JAR_SHARD i/4), DexScreener-creator scope (config v7)
cd /root/sol
pkill -f "release/jaringan devtrace"
sleep 2
for i in 0 1 2 3; do
  JAR_SHARD=$i/4 JAR_RPS=3 nohup ./target/release/jaringan devtrace data/v3/universe data/v3/dev 1 >> data/v3/dev/run_$i.log 2>&1 < /dev/null &
done
