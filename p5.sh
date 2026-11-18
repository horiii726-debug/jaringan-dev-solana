#!/bin/sh
# Waits for P3 + random trades, then: network-coin trades (within budget), outcomes, backtest, post-mortem.
cd /root/jaringan
until grep -q P3_DONE logs/p3.log 2>/dev/null && grep -q TRADES_DONE logs/trades_rand.log 2>/dev/null; do sleep 30; done
L=data/launches_2026-11-14.jsonl,data/launches_2026-11-15.jsonl
F=data/p3/fund_s1h.jsonl,data/p3/fund_d1a.jsonl,data/p3/fund_d1.jsonl,data/p3/fund_d2.jsonl
cat data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.jsonl > data/p2/trades_rand.jsonl
# network coins not yet pulled, random order, as many as the budget allows (25 credits each, 10k reserve)
USED=$(cat checkpoints/credits/*.txt | awk "{s+=\$1} END {print s}")
CAP=$(grep max_provider_credits config/budget.toml | tr -dc 0-9)
N=999999
awk -F, "\$3==\"creator\"{print \$1}" data/p3/networks_d1415.csv | sort -u > /tmp/nc.txt
jq -r .mint data/p2/trades_rand.jsonl | sort -u > /tmp/have.txt
awk -F, "FILENAME==ARGV[1]{c[\$1]=1;next} FILENAME==ARGV[2]{h[\$1]=1;next} 1" /tmp/nc.txt /tmp/have.txt /dev/null > /dev/null
jq -r "select(.kind==\"create\" and .mayhem==false and (.quote|startswith(\"1111\")))|[.mint,.curve,.bt,.creator]|@csv" data/launches_2026-11-14.jsonl data/launches_2026-11-15.jsonl | tr -d "\"" \
 | awk -F, "FILENAME==ARGV[1]{c[\$1]=1;next} FILENAME==ARGV[2]{h[\$1]=1;next} (\$4 in c) && !(\$1 in h){print \$1\",\"\$2\",\"\$3}" /tmp/nc.txt /tmp/have.txt - \
 | shuf --random-source=/root/jaringan/config/labels.toml | head -n $N > data/p2/net_coins.txt
echo "network coins to pull: $(wc -l < data/p2/net_coins.txt) (budget N=$N)"
./trades_par.sh data/p2/net_coins.txt data/p2/trades_net 12
./target/release/jaringan outcomes data/p2/outcomes_rand.csv $L data/p2/trades_rand.jsonl
./target/release/jaringan outcomes data/p2/outcomes_all.csv $L data/p2/trades_rand.jsonl,data/p2/trades_net.jsonl
for d in 0o 0 1 2 5 13; do
  ./target/release/jaringan bt rand data/p2/outcomes_rand.csv $d $L $F
  ./target/release/jaringan bt all data/p2/outcomes_all.csv $d $L $F
  ./target/release/jaringan pm all data/p2/outcomes_all.csv data/p5/scored_all_d$d.csv $d
done
echo P5_DONE
