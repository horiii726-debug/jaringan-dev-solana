#!/bin/bash
# 18 Nov: one heavy job at a time (VPS froze when many ran together).
cd /root/jaringan
log() { echo "$(date -u +%H:%M) $*"; }
log "1 devhist oldest-first, all DexScreener devs"
JAR_ASC=1 ./target/release/jaringan devhist data/family/devs.txt 3 data/family/devhist_asc.jsonl | tail -1
log "2 merge batch-1 crawl into main dir (resume, no double credits)"
for f in wallets edges creates; do [ -s data/family/$f.jsonl ] || cp data/family_a/$f.jsonl data/family/$f.jsonl; done
log "3 family: up for the rest, then down"
./target/release/jaringan family data/family/devs.txt data/family 10000 | tail -3
log "4 dead coins, rug, export"
./target/release/jaringan famdead data/family
./target/release/jaringan trades data/family/dead.csv data/family/dead_trades.jsonl | tail -1
./target/release/jaringan deadcls data/family data/family/dead_trades.jsonl > data/family/deadcls.out
./target/release/jaringan devsum data/family/devhist_asc.jsonl data/family/up.jsonl > data/family/devsum.out
./target/release/jaringan famexport data/family
log PIPELINE_DONE
