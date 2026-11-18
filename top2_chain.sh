#!/bin/bash
cd /root/jaringan
while pgrep -f "release/jaringan trades data/gml" >/dev/null; do sleep 10; done
./top2_run.sh > data/gml/top2b.log 2>&1
