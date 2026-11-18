#!/bin/sh
# Build, export teams for the web page, start the local web server (127.0.0.1:8080).
cd /root/jaringan
. /root/.cargo/env
cargo clippy --release --all-targets -- -D warnings > logs/web_clippy.log 2>&1
cargo build --release > logs/web_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl 2>/dev/null | paste -sd,)
F=$(ls data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p3/fund_d2.part_*.jsonl 2>/dev/null | paste -sd,)
L=data/launches_2026-11-14.jsonl,data/launches_2026-11-15.jsonl
./target/release/jaringan outcomes data/p4/outcomes_team.csv $L $T > /dev/null
./target/release/jaringan team $L $T $F data/p4/outcomes_team.csv j1 0.0146
tmux kill-session -t web 2>/dev/null
tmux new -d -s web "./target/release/jaringan serve 8899"
echo WEB_DONE
