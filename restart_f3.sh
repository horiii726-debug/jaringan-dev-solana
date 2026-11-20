#!/usr/bin/env bash
# 21 Nov: athgt formula 3 (SOL-quoted pools only, confirmed ATH, sanity checks) -> archive formula 2 rows, restart
cd /root/sol
pkill -f "release/jaringan athgt"
sleep 2
mkdir -p data/v3/ath/old_formula2
mv data/v3/ath/ath_[0-9]*.jsonl data/v3/ath/old_formula2/ 2>/dev/null
bash start_ath.sh
# validation set (same 17 coins as the formula-2 check) into its own folder
L=$(jq -r "select(.ath_mcap_usd) | .mint" data/v3/ath/ath_gt_uji.jsonl | sort -u | paste -sd,)
mkdir -p data/v3/ath_val3
(JAR_ATH_ONLY=$L JAR_ATH_PAGES=300 JAR_RPS=3 timeout 900 ./target/release/jaringan athgt data/v3/universe data/v3/dev data/v3/ath_val3 > data/v3/ath_val3/run.log 2>&1 &)
