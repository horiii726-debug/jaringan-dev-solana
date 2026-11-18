#!/bin/sh
# Priority trace for devs of rising coins: their funder, the funder of that funder, funder activity, then scoreboard.
cd /root/jaringan
. /root/.cargo/env
cargo build --release > logs/rising_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
./fund_par.sh data/p4/rising_devs_todo.txt data/p4/fund_rise 4
cat data/p4/fund_rise.jsonl data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl 2>/dev/null | jq -r .w | sort -u > /tmp/fd.txt
cat data/p4/fund_rise.jsonl data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl 2>/dev/null | jq -r "select(.funder!=null)|[.w,.funder]|@tsv" | gawk -F"\t" "ARGIND==1{r[\$1]=1;next} (\$1 in r){print \$2}" data/p4/rising_devs.txt - | sort -u > /tmp/rf.txt
comm -23 /tmp/rf.txt /tmp/fd.txt > data/p4/rising_funders_todo.txt
./fund_par.sh data/p4/rising_funders_todo.txt data/p4/fund_rise2 4
cat /tmp/rf.txt data/p3/funders_ge2.txt | sort -u > data/p4/act_list.txt
./target/release/jaringan activity data/p4/act_list.txt data/p3/activity.jsonl
T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl 2>/dev/null | paste -sd,)
F=$(ls data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_rise.jsonl data/p4/fund_rise2.jsonl 2>/dev/null | paste -sd,)
L=data/launches_2026-11-14.jsonl,data/launches_2026-11-15.jsonl
./target/release/jaringan outcomes data/p4/outcomes_team.csv $L $T > /dev/null
./target/release/jaringan score $L $T $F data/p4/outcomes_team.csv
echo RISING_DONE
