#!/bin/sh
cd /root/jaringan
. /root/.cargo/env
while pgrep -f "[c]argo build" > /dev/null; do sleep 5; done
cargo clippy --release --all-targets -- -D warnings > logs/cats_clippy.log 2>&1
cargo build --release > logs/cats_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
./ml_cats.sh
