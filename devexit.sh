#!/bin/bash
# Exit with the dev family (no fixed holding time) for the walk-forward model picks.
cd /root/jaringan || exit 1
. /root/.cargo/env
cargo clippy --release --all-targets -- -D warnings > logs/devexit_clippy.log 2>&1
cargo build --release > logs/devexit_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
P=data/ml/wf/top50_all.txt
jq -r '[.mint, ("pool="+.pool), .slot, .bt]|@csv' data/p2amm/migrations.jsonl | tr -d '"' | gawk -F, 'ARGIND==1{t[$1]=1;next} ($1 in t)' $P - | sort -u -t, -k1,1 > data/p2amm/pick_pools.txt
sed -i "s/^amm_max_pages = .*/amm_max_pages = 3/" config/budget.toml
./target/release/jaringan amm data/p2amm/pick_pools.txt data/p2amm/amm_dev.jsonl > logs/amm_dev.log 2>&1
sed -i "s/^amm_max_pages = .*/amm_max_pages = 10/" config/budget.toml
jq -r 'select(.kind=="create")|[.mint,.creator,.user,.bt]|@csv' data/launches_2026-1*.jsonl | tr -d '"' | gawk -F, 'ARGIND==1{t[$1]=1;next} ($1 in t)' $P - > data/ml/pick_devs.csv
./target/release/jaringan devflow data/ml/pick_devs.csv data/ml/devflow.jsonl > logs/devflow.log 2>&1
L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
T=$(ls data/p2/tr_*.jsonl data/p2/tr_*.part_*.jsonl 2>/dev/null | paste -sd,)
JAR_FAMILY=data/ml/devflow.jsonl JAR_SIM_LIST=$P ./target/release/jaringan gradsim $L $T data/p2amm/amm_dev.jsonl 0.0146 71.08 > /dev/null 2>&1
jq -c '.list|{koin:.n, dev_jual_terdeteksi:.dev_sold, menang:.wdev, median:.rdev_median, rata:.rdev, rata_tanpa_top5:.rdev_no_top5, pembanding_tahan_1jam_median:.r1h_median}' reports/web/grad.json
jq -r '.list_rows[]|[.bt,.r_dev,.t_exit]|@tsv' reports/web/grad.json | sort -n > data/ml/dev_trades.tsv
echo DEVEXIT_DONE
