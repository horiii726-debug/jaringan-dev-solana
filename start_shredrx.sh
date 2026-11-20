#!/usr/bin/env bash
# ShredStream gRPC receiver (PAPER: detect Pump.fun creates + watchlist hits), log data/shred/rx.log
cd /root/sol
pkill -f "shredrx/target/release/shredrx"
sleep 1
nohup ./shredrx/target/release/shredrx >> data/shred/rx.log 2>&1 < /dev/null &
