#!/usr/bin/env bash
# measure rises of every stream-caught coin, looping so new coins are picked up
cd /root/sol
while true; do
  JAR_RPS=3 ./target/release/jaringan streamrise data/shred/creates.jsonl data/shred/rise 5 >> data/shred/rise_run.log 2>&1
  sleep 60
done
