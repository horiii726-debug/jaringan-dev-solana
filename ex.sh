#!/bin/sh
cd /root/jaringan
. /root/.cargo/env
cargo clippy --release --all-targets -- -D warnings > logs/ex_clippy.log 2>&1
cargo build --release > logs/ex_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl data/p2/tr_2026-*.part_*.jsonl 2>/dev/null | paste -sd,)
F=$(ls data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | paste -sd,)
./target/release/jaringan outcomes data/p4/outcomes_all.csv $L $T > /dev/null
./target/release/jaringan team $L $T $F data/p4/outcomes_all.csv j1 0.0146 | grep -E "busy|teams with|SIGNALS"
./target/release/jaringan score $L $T $F data/p4/outcomes_all.csv
tmux kill-session -t web 2>/dev/null; tmux new -d -s web "./target/release/jaringan serve 8899"
echo EX_DONE
