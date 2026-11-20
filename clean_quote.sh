#!/usr/bin/env bash
# 21 Nov: pools quoted in a non-SOL token (USDC / other pump tokens) were priced as if quoted in SOL -> drop those rows
cd /root/sol
pkill -f "release/jaringan athgt"
sleep 2
mkdir -p data/v3/ath/bak_quote
for f in data/v3/ath/ath_[0-9]*.jsonl; do
  cp "$f" data/v3/ath/bak_quote/
  jq -c --rawfile bad data/v3/ath/quote_bukan_sol.txt '($bad | split("\n")) as $b | select((.mint as $m | $b | index($m)) | not)' "data/v3/ath/bak_quote/$(basename $f)" > "$f"
done
cat data/v3/ath/bak_quote/ath_*.jsonl | wc -l
cat data/v3/ath/ath_[0-9]*.jsonl | wc -l
