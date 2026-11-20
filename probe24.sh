#!/usr/bin/env bash
# ShredStream trial: keep the UDP 8001 probe up for 24 h (activation is manual via Discord, may take hours)
cd /root/sol
pkill -f "release/jaringan udpprobe"
sleep 1
nohup ./target/release/jaringan udpprobe 8001 86400 > data/shred/probe.log 2>&1 < /dev/null &
