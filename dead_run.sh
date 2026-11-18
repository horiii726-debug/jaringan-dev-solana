#!/bin/bash
cd /root/jaringan
./target/release/jaringan trades data/family_a/dead.csv data/family_a/dead_trades.jsonl
./target/release/jaringan deadcls data/family_a data/family_a/dead_trades.jsonl > data/family_a/deadcls.out
echo DEAD_DONE
