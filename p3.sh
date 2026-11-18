#!/bin/sh
# P3: funders of creators+signers (days 14-15), then funders of funders (depth 2), then graph. Resumable.
cd /root/jaringan
done_list() { cat data/p3/fund_s1h.jsonl data/p3/fund_d1a.jsonl data/p3/fund_d1.jsonl data/p3/fund_d2.jsonl 2>/dev/null | jq -r .w | sort -u; }
jq -r "select(.kind==\"create\")|.creator,.user" data/launches_2026-11-14.jsonl data/launches_2026-11-15.jsonl | sort -u > /tmp/w.txt
done_list > /tmp/done.txt
comm -23 /tmp/w.txt /tmp/done.txt > data/p3/w_d1.txt
echo "depth1 wallets $(wc -l < data/p3/w_d1.txt)"
./fund_par.sh data/p3/w_d1.txt data/p3/fund_d1 8
cat data/p3/fund_s1h.jsonl data/p3/fund_d1a.jsonl data/p3/fund_d1.jsonl | jq -r "select(.funder!=null)|.funder" | sort -u > /tmp/f.txt
done_list > /tmp/done2.txt
comm -23 /tmp/f.txt /tmp/done2.txt > data/p3/w_d2.txt
echo "depth2 wallets $(wc -l < data/p3/w_d2.txt)"
./fund_par.sh data/p3/w_d2.txt data/p3/fund_d2 8
./target/release/jaringan graph d1415 data/launches_2026-11-14.jsonl,data/launches_2026-11-15.jsonl data/p3/fund_s1h.jsonl,data/p3/fund_d1a.jsonl,data/p3/fund_d1.jsonl,data/p3/fund_d2.jsonl > /dev/null
echo P3_DONE
