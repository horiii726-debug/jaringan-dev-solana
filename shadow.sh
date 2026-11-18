#!/bin/bash
cd /root/jaringan
. /root/.cargo/env
until grep -q "DEVEXIT_DONE\|BUILD_FAIL" logs/devexit.log; do sleep 15; done
grep -q "mod shadow;" src/main.rs || { sed -i "s/^mod devflow;/mod devflow;\nmod shadow;/" src/main.rs; sed -i "s|        Some(\"graph\")|        Some(\"shadow\") if a.len() == 4 => shadow::run(\&a[2], \&a[3]),\n        Some(\"graph\")|" src/main.rs; }
cargo clippy --release --all-targets -- -D warnings > logs/shadow_clippy.log 2>&1
cargo build --release > logs/shadow_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
set -a; . ./.env; set +a
JAR_RPC_URL="https://rpc.shyft.to?api_key=$SHYFT_KEY" JAR_RPS=5 ./target/release/jaringan shadow reports/web/best_teams.json reports/shadow
