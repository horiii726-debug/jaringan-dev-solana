#!/bin/sh
cd /root/jaringan
. /root/.cargo/env
cargo clippy --release --all-targets -- -D warnings > logs/amm_clippy.log 2>&1
cargo build --release > logs/amm_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
mkdir -p data/p2amm
cat data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl 2>/dev/null | jq -r "select(.graduated_slot!=null)|. as \$x|[.mint, .graduated_slot, ([.t[]|select(.[0]==\$x.graduated_slot)|.[3]]|first // .cbt)]|@tsv" | sort -u > /tmp/grad.tsv
jq -r "select(.kind==\"create\")|[.mint,.curve]|@tsv" data/launches_2026-11-1*.jsonl | gawk -F"\t" "ARGIND==1{g[\$1]=\$2\",\"\$3;next} (\$1 in g){print \$1\",\"\$2\",\"g[\$1]}" /tmp/grad.tsv - > data/p2amm/grad_list.txt
echo "graduated coins: $(wc -l < data/p2amm/grad_list.txt)"
./target/release/jaringan amm data/p2amm/grad_list.txt data/p2amm/amm2.jsonl
echo AMM_DONE
