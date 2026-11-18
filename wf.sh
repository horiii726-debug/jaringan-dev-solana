#!/bin/bash
# Walk-forward, day by day (all on the VPS): $50k model (CatBoost) and the rule algorithm.
# For test day D: train = days 06..D-3, valid = D-2, test = D (D-1 purged: labels settle within ~1 day).
# Then the top-50 model picks of each test day are simulated: Jito-first 0.1 SOL, tip, hold 1 h.
cd /root/jaringan || exit 1
CB=/root/bin/catboost; M=data/ml; W=$M/wf; TAB=$(printf '\t'); mkdir -p $W
auc() { sort -t"$TAB" -k2,2g | awk -F"$TAB" '{n++; if($1==1){p++; r+=n}} END{q=n-p; if(p&&q) printf "%.3f", (r-p*(p+1)/2)/(p*q); else printf "-"}'; }
: > $W/top50_all.txt
echo "dag | train(pos) | test(pos) | AUC | top50 hit | top100 hit | ALGORITMA (dev 0.5-2 SOL, bukan 1-2 koin lama): n hit%"
for D in 2026-11-09 2026-11-10 2026-11-11 2026-11-12 2026-11-13 2026-11-14 2026-11-15; do
  V=$(date -u -d "$D - 2 day" +%F); T3=$(date -u -d "$D - 3 day" +%F)
  awk -F"$TAB" -v e=$T3 'NR==1 || ($2>="2026-11-06" && $2<=e)' $M/fall.tsv > $W/tr.tsv
  awk -F"$TAB" -v v=$V 'NR==1 || $2==v' $M/fall.tsv > $W/va.tsv
  awk -F"$TAB" -v d=$D 'NR==1 || $2==d' $M/fall.tsv > $W/te_$D.tsv
  $CB fit --learn-set $W/tr.tsv --test-set $W/va.tsv --cd $M/fall.tsv.cd --has-header --loss-function Logloss --eval-metric PRAUC \
    --auto-class-weights SqrtBalanced --iterations 2000 --learning-rate 0.05 --depth 6 --od-type Iter --od-wait 200 --use-best-model \
    --random-seed 42 --train-dir $W/cb_$D -m model.cbm --logging-level Silent > $W/fit_$D.log 2>&1 || { echo "$D fit failed"; continue; }
  $CB calc -m $W/cb_$D/model.cbm --input-path $W/te_$D.tsv --cd $M/fall.tsv.cd --has-header --output-path $W/pr_$D.tsv --prediction-type Probability > /dev/null 2>&1
  paste <(tail -n +2 $W/te_$D.tsv | cut -f1,4) <(tail -n +2 $W/pr_$D.tsv | awk -F"$TAB" '{print $NF}') > $W/lp_$D.tsv
  A=$(cut -f1,3 $W/lp_$D.tsv | auc)
  TOP=$(sort -t"$TAB" -k3,3gr $W/lp_$D.tsv | awk -F"$TAB" 'NR<=50{a+=$1} NR<=100{b+=$1} END{print a" / "b}')
  sort -t"$TAB" -k3,3gr $W/lp_$D.tsv | head -50 | cut -f2 >> $W/top50_all.txt
  ALG=$(awk -F"$TAB" 'NR>1 && $5>=0.5 && $5<2 && !($14>=1 && $14<=2){n++; h+=$1} END{printf "%d %.2f%%", n, (n?100*h/n:0)}' $W/te_$D.tsv)
  echo "$D | $(($(wc -l < $W/tr.tsv)-1))($(awk -F"$TAB" 'NR>1&&$1==1' $W/tr.tsv | wc -l)) | $(($(wc -l < $W/te_$D.tsv)-1))($(awk -F"$TAB" 'NR>1&&$1==1' $W/te_$D.tsv | wc -l)) | $A | $TOP | $ALG"
done
echo "== simulasi beli (top-50 per hari uji): tarik trade lalu Jito-first hold 1 jam"
jq -r "select(.kind==\"create\")|[.mint,.curve,.bt]|@csv" data/launches_2026-1*.jsonl | tr -d '"' | gawk -F, 'ARGIND==1{t[$1]=1;next} ($1 in t)' $W/top50_all.txt - > $W/top50.csv
cat data/p2/tr_*.jsonl data/p2/tr_*.part_*.jsonl 2>/dev/null | jq -r .mint | sort -u > /tmp/have_all.txt
gawk -F, 'ARGIND==1{h[$1]=1;next} !($1 in h)' /tmp/have_all.txt $W/top50.csv > $W/top50_need.csv
./target/release/jaringan trades $W/top50_need.csv data/p2/tr_wf.jsonl > /dev/null 2>&1
L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
T=$(ls data/p2/tr_*.jsonl data/p2/tr_*.part_*.jsonl 2>/dev/null | grep -v "\.txt" | paste -sd,)
JAR_SIM_LIST=$W/top50_all.txt ./target/release/jaringan gradsim $L $T data/p2amm/amm_all.jsonl 0.0146 71.08 > /dev/null 2>&1
jq -c ".list|{koin:.n, menang_1jam:.w1h, median_1jam:.r1h_median, rata_1jam:.r1h, rata_tanpa_top5:.r1h_no_top5}" reports/web/grad.json
echo WF_DONE
