#!/bin/bash
# syndicate scan for creators of the 5,992 DEX coins: strict first funding (>= 0.05 SOL direct transfer), funders with >= 2
# creators labelled (exchange / service / bot are not founders), then one more level up for operators.
cd /root/jaringan || exit 1
D=data/roots
./target/release/jaringan fund $D/ds_creators.txt $D/ds_fund_l1.jsonl > /dev/null 2>&1
jq -r "select(.funder!=null and .kind==\"transfer\" and (.lam//0)>=50000000)|.funder" $D/ds_fund_l1.jsonl | sort | uniq -c | awk "\$1>=2{print \$2}" > $D/ds_f1_multi.txt
./target/release/jaringan svc $D/ds_f1_multi.txt $D/svc.jsonl > /dev/null 2>&1
jq -r "select(.label==\"OPERATOR\")|.w" $D/svc.jsonl | sort -u | grep -xF -f $D/ds_f1_multi.txt > $D/ds_lvl2.txt
./target/release/jaringan fund $D/ds_lvl2.txt $D/ds_fund_l2.jsonl > /dev/null 2>&1
echo SYND_DONE
