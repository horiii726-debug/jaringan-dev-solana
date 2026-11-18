#!/bin/bash
# Build dataset/ (compressed derived + raw data) for the GitHub backup. Secrets never included.
cd /root/jaringan
mkdir -p dataset/{teams,coins,wallets,launches,trades,ml}   # never delete old files; only add or update
cp reports/teams_terbaik.tsv reports/teams_big.tsv reports/team_members.tsv reports/watch_wallets.tsv dataset/teams/ 2>/dev/null
cp reports/web/best_teams.json reports/web/score.json reports/web/teams.json reports/web/cats.json reports/web/grad.json dataset/teams/ 2>/dev/null
cp data/p2amm/min50k_fix2.tsv data/p2amm/peak_mcap2.tsv dataset/coins/ 2>/dev/null
gzip -c data/p2amm/migrations.jsonl > dataset/coins/migrations.jsonl.gz
gzip -c data/ml/cats_all.tsv > dataset/coins/categories.tsv.gz
gzip -c data/p4/outcomes_all.csv > dataset/coins/outcomes_j1.csv.gz 2>/dev/null
cp data/bq/top_tokens.tsv data/bq/big_bq.tsv data/bq/big_creators.tsv dataset/coins/ 2>/dev/null
cat data/p3/fund_*.jsonl data/p3/fund_*.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | jq -r "select(.funder!=null)|[.w,.funder,(.first_bt//0),(.kind//\"\"),(.n_sigs//0)]|@tsv" | sort -u | gzip > dataset/wallets/funding_edges.tsv.gz
gzip -c data/p4/crawl.jsonl > dataset/wallets/bubblemap_edges.jsonl.gz
cp data/p3/exchange_list.txt data/p4/profile.jsonl data/p3/activity.jsonl data/p4/balances_21teams.tsv data/ml/devflow.jsonl dataset/wallets/ 2>/dev/null
for f in data/launches_2026-*.jsonl; do gzip -c $f > dataset/launches/$(basename $f).gz; done
for f in data/p2/tr_*.jsonl data/p2/tr_*.part_*.jsonl; do [ -s "$f" ] && gzip -c $f > dataset/trades/$(basename $f).gz; done
gzip -c data/p2amm/amm_all.jsonl > dataset/trades/pumpswap_amm_all.jsonl.gz
cp data/ml/cb_big/model.cbm dataset/ml/model_50k.cbm 2>/dev/null; cp reports/ml/*.txt dataset/ml/ 2>/dev/null; cp data/ml/wf_trades.tsv dataset/ml/ 2>/dev/null
find dataset -type f -size +95M -exec ls -la {} \;
du -sh dataset
