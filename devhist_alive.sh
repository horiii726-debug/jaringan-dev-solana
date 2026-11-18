#!/bin/bash
# Devs of the 819 alive DEX coins: (1) first funding (strict) + label of the funder, (2) full launch history
# (up to 5 pages = 500 txs per wallet; longer histories flagged incomplete).
cd /root/jaringan || exit 1
D=data/roots
./target/release/jaringan fund $D/alive_devs.txt $D/alive_fund.jsonl > /dev/null 2>&1
jq -r "select(.funder!=null)|.funder" $D/alive_fund.jsonl | sort -u > $D/alive_funders.txt
./target/release/jaringan svc $D/alive_funders.txt $D/svc.jsonl > /dev/null 2>&1
./target/release/jaringan devhist $D/alive_devs.txt 5 $D/alive_hist.jsonl > /dev/null 2>&1
echo DEVHIST_DONE
