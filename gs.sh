#!/bin/sh
cd /root/jaringan
. /root/.cargo/env
until grep -q "AMM_DONE\|BUILD_FAIL" logs/amm.log; do sleep 30; done
cargo clippy --release --all-targets -- -D warnings > logs/gs_clippy.log 2>&1
cargo build --release > logs/gs_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl 2>/dev/null | paste -sd,)
./target/release/jaringan gradsim $L $T data/p2amm/amm2.jsonl 0.0146 71.08
echo GS_DONE
