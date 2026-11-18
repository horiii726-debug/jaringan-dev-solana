#!/bin/sh
cd /root/jaringan
. /root/.cargo/env
cargo build --release > logs/prof_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
./target/release/jaringan profile data/p4/roots_moon.txt data/p4/all_creators.txt data/p4/profile.jsonl
echo PROF_DONE
