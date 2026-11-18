#!/bin/bash
cd /root/jaringan || exit 1
while pgrep -f "[j]aringan svc /tmp/lab_need" >/dev/null; do sleep 15; done
D=data/roots
jq -r "[.w,.label]|@tsv" $D/svc.jsonl > $D/labels.tsv
{ jq -r "select(.funder!=null and .kind==\"transfer\" and (.lam//0)>=50000000)|[.w,.funder,\"F\"]|@tsv" $D/ds_fund_l1.jsonl
  jq -r ".w as \$w | .inflow_before[] | select(.sol>=0.05) | [\$w,.from,\"I\"]|@tsv" $D/alive_devprof.jsonl
  jq -r ".w as \$w | .outflow_after[] | select(.sol>=0.1) | [\$w,.to,\"O\"]|@tsv" $D/alive_devprof.jsonl
  jq -r ".w as \$w | [\$w, \$w, \"-\"]|@tsv" $D/alive_devprof.jsonl
  jq -r "[.w,.w,\"-\"]|@tsv" $D/ds_fund_l1.jsonl; } > $D/edges.tsv
gawk -f clus.awk $D/labels.tsv $D/edges.tsv 2> $D/synd_list.tsv
sort $D/synd_list.tsv | uniq -c | sort -rn | head -15
echo CLUS_DONE
