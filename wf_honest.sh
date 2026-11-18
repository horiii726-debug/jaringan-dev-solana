#!/bin/bash
# F6 step 2 (18 Nov review): walk-forward $50k model WITHOUT funding-coverage features (has_fund, fund_lag_s, team_*),
# 6 Nov excluded (partial day). Train = 07..D-3, valid = D-2, test = D (D-1 purged). Plus label-shuffle attack.
# Top-50 picks per test day -> honest sim (reachable entry, honest value): reports/honest/wf_*.
cd /root/jaringan || exit 1
CB=/root/bin/catboost; M=data/ml; W=$M/wf_honest; TAB=$(printf "\t"); mkdir -p $W reports/honest
SHUF=${SHUF:-0}
printf "0\tLabel\n1\tAuxiliary\n2\tAuxiliary\n3\tAuxiliary\n18\tAuxiliary\n19\tAuxiliary\n22\tAuxiliary\n23\tAuxiliary\n24\tAuxiliary\n" > $W/cd
auc() { sort -t"$TAB" -k2,2g -s | awk -F"$TAB" -f /root/jaringan/auc_ties.awk; }
: > $W/top50_s$SHUF.txt
echo "day | train(pos) | test(pos) | AUC | top50 hit | top100 hit | shuffle=$SHUF"
for D in 2026-11-10 2026-11-11 2026-11-12 2026-11-13 2026-11-14 2026-11-15; do
  V=$(date -u -d "$D - 2 day" +%F); T3=$(date -u -d "$D - 3 day" +%F)
  awk -F"$TAB" -v e=$T3 "NR==1 || (\$2>=\"2026-11-07\" && \$2<=e)" $M/fall.tsv > $W/tr.tsv
  if [ "$SHUF" = 1 ]; then { head -1 $W/tr.tsv; paste <(tail -n +2 $W/tr.tsv | cut -f1 | shuf --random-source=<(yes)) <(tail -n +2 $W/tr.tsv | cut -f2-); } > $W/tr2.tsv; mv $W/tr2.tsv $W/tr.tsv; fi
  awk -F"$TAB" -v v=$V "NR==1 || \$2==v" $M/fall.tsv > $W/va.tsv
  awk -F"$TAB" -v d=$D "NR==1 || \$2==d" $M/fall.tsv > $W/te_$D.tsv
  $CB fit --learn-set $W/tr.tsv --test-set $W/va.tsv --cd $W/cd --has-header --loss-function Logloss --eval-metric PRAUC --task-type GPU \
    --auto-class-weights SqrtBalanced --iterations 2000 --learning-rate 0.05 --depth 6 --od-type Iter --od-wait 200 --use-best-model \
    --random-seed 42 --train-dir $W/cb_$D -m model.cbm --logging-level Silent > $W/fit_$D.log 2>&1 || { echo "$D fit failed"; tail -3 $W/fit_$D.log; continue; }
  $CB calc -m $W/cb_$D/model.cbm --input-path $W/te_$D.tsv --cd $W/cd --has-header --output-path $W/pr_$D.tsv --prediction-type Probability > /dev/null 2>&1
  paste <(tail -n +2 $W/te_$D.tsv | cut -f1,4) <(tail -n +2 $W/pr_$D.tsv | awk -F"$TAB" "{print \$NF}") > $W/lp_$D.tsv
  A=$(cut -f1,3 $W/lp_$D.tsv | auc)
  TOP=$(sort -t"$TAB" -k3,3gr $W/lp_$D.tsv | awk -F"$TAB" "NR<=50{a+=\$1} NR<=100{b+=\$1} END{print a\" | \"b}")
  sort -t"$TAB" -k3,3gr $W/lp_$D.tsv | head -50 | cut -f2 >> $W/top50_s$SHUF.txt
  echo "$D | $(($(wc -l < $W/tr.tsv)-1))($(awk -F"$TAB" "NR>1&&\$1==1" $W/tr.tsv | wc -l)) | $(($(wc -l < $W/te_$D.tsv)-1))($(awk -F"$TAB" "NR>1&&\$1==1" $W/te_$D.tsv | wc -l)) | $A | $TOP"
done
