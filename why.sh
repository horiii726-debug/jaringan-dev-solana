#!/bin/bash
cd /root/jaringan
./ml_cats.sh > logs/ml_cats.log 2>&1
jq -r ".teams[].dev_list[]" reports/web/score.json | sort -u > /tmp/teamdevs.txt
jq -r "select(.kind==\"create\")|[.creator,.mint]|@tsv" data/launches_2026-1*.jsonl | gawk -F"\t" "ARGIND==1{d[\$1]=1;next} (\$1 in d){print \$2}" /tmp/teamdevs.txt - > /tmp/teammints.txt
gawk -f why.awk data/ml/cats.tsv /tmp/teammints.txt data/ml/feat_dj1.tsv > reports/ml/why.txt
echo WHY_DONE
