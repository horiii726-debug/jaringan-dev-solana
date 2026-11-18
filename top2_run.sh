#!/bin/bash
cd /root/jaringan
./target/release/jaringan trades data/gml/top2.csv data/gml/top2_trades.jsonl
echo TOP2_DONE
