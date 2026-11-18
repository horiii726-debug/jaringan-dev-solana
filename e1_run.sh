#!/bin/bash
cd /root/jaringan
./target/release/jaringan trades data/family_a/e1.csv data/family_a/e1_trades.jsonl
echo TRADES_DONE
