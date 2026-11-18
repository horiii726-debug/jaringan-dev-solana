#!/bin/sh
cd /root/jaringan
. /root/.cargo/env
until grep -q "CATS_DONE\|BUILD_FAIL" logs/cats.log 2>/dev/null; do sleep 10; done
cargo clippy --release --all-targets -- -D warnings > logs/web2_clippy.log 2>&1
cargo build --release > logs/web2_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
tmux kill-session -t web 2>/dev/null
tmux new -d -s web "./target/release/jaringan serve 8899"
echo WEB2_DONE
