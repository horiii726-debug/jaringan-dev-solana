#!/usr/bin/env bash
# restart fam2 x3 (DexScreener-creator scope) after an Aux provider change
cd /root/sol
pkill -f "release/jaringan fam2"
sleep 2
export JAR_SCOPE_FILE=/root/sol/data/v3/reput/ds_creators.txt
for i in 0 1 2; do
  JAR_SHARD=$i/3 JAR_RPS=2 nohup ./target/release/jaringan fam2 data/v3/dev data/v3/fam2 1 >> data/v3/fam2/run_$i.log 2>&1 < /dev/null &
done
