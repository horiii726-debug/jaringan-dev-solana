#!/bin/bash
# Multiclass CatBoost: predict the coin category (MOONSHOT/NAIK/TURUN/RUGPULL/SEPI) from facts known when we would
# enter (after 1 sniper, same block: create tx + history only). Train day 14 (valid = last 20%), test day 15.
# Writes reports/ml/cats_run.txt and reports/web/mlcats.json (confusion matrix, per-class precision/recall, loss curve).
cd /root/jaringan || exit 1
CB=/root/bin/catboost; M=data/ml; TAB=$(printf '\t'); D=j1
L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl data/p2/tr_2026-*.part_*.jsonl data/p2/tr_emx.jsonl 2>/dev/null | paste -sd,)
F=$(ls data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | paste -sd,)
./target/release/jaringan outcomes $M/outcomes.csv data/launches_2026-11-14.jsonl,data/launches_2026-11-15.jsonl $T > /dev/null
./target/release/jaringan feat $L $T $F $M/outcomes.csv $D $M/feat_d$D.tsv
./target/release/jaringan cats data/launches_2026-11-14.jsonl,data/launches_2026-11-15.jsonl $T $M/cats.tsv
NC=$(head -1 $M/feat_d$D.tsv | awk -F"$TAB" '{print NF}')
# replace column 1 (binary label) by the category, keep rows that have a category
awk -F"$TAB" -v OFS="$TAB" -v nc=$NC 'NR==FNR{c[$1]=$2; next} FNR==1{$1="cat"; print; next} ($nc in c){$1=c[$nc]; print}' $M/cats.tsv $M/feat_d$D.tsv > $M/featc.tsv
H=$(head -1 $M/featc.tsv)
T0=$(awk -F"$TAB" 'NR>1 && $4=="2026-11-15"{print $5}' $M/featc.tsv | sort -n | head -1)
awk -F"$TAB" -v t="$T0" 'NR>1 && $4=="2026-11-14" && $5+1800<=t' $M/featc.tsv | sort -t"$TAB" -k5,5n > /tmp/ctr.tsv
N=$(wc -l < /tmp/ctr.tsv); NV=$((N/5))
{ echo "$H"; head -n $((N-NV)) /tmp/ctr.tsv; } > $M/ctrain.tsv
{ echo "$H"; tail -n $NV /tmp/ctr.tsv; } > $M/cvalid.tsv
awk -F"$TAB" 'NR==1 || $4=="2026-11-15"' $M/featc.tsv > $M/ctest.tsv
mkdir -p $M/cb_cats
$CB fit --learn-set $M/ctrain.tsv --test-set $M/cvalid.tsv --cd $M/feat_d$D.tsv.cd --has-header --loss-function MultiClass \
  --class-names MOONSHOT,NAIK,TURUN,RUGPULL,SEPI --auto-class-weights Balanced --iterations 2000 --learning-rate 0.05 --depth 6 \
  --od-type Iter --od-wait 200 --use-best-model --task-type GPU --devices 0 --random-seed 42 --train-dir $M/cb_cats -m model.cbm --logging-level Silent > $M/cb_cats.log 2>&1 || { tail -3 $M/cb_cats.log; exit 1; }
$CB calc -m $M/cb_cats/model.cbm --input-path $M/ctest.tsv --cd $M/feat_d$D.tsv.cd --has-header --output-path $M/cpred.tsv --prediction-type Class > /dev/null 2>&1
{
echo "rows train=$((N-NV)) valid=$NV test=$(($(wc -l < $M/ctest.tsv)-1))"
echo "== loss curve MultiClass: iter | train | valid"
paste $M/cb_cats/learn_error.tsv $M/cb_cats/test_error.tsv | awk -F"$TAB" 'NR==1{next} (NR-2)%100==0{printf "%s | %.4f | %.4f\n",$1,$2,$4}' | head -20
echo "== confusion (actual -> predicted) on test day"
paste <(tail -n +2 $M/ctest.tsv | cut -f1) <(tail -n +2 $M/cpred.tsv | awk -F"$TAB" '{print $NF}') | sort | uniq -c | sort -k2,2 -k3,3
} > reports/ml/cats_run.txt
# JSON for the web page
paste <(tail -n +2 $M/ctest.tsv | cut -f1) <(tail -n +2 $M/cpred.tsv | awk -F"$TAB" '{print $NF}') | awk -F"$TAB" '
{n[$1"|"$2]++; a[$1]++; p[$2]++; t++}
END{ split("MOONSHOT NAIK TURUN RUGPULL SEPI",C," "); printf "{\"test_n\":%d,\"classes\":[",t;
 for(i=1;i<=5;i++){c=C[i]; tp=n[c"|"c]+0; pr=(p[c]?tp/p[c]:0); rc=(a[c]?tp/a[c]:0);
  printf "%s{\"cat\":\"%s\",\"actual\":%d,\"predicted\":%d,\"correct\":%d,\"precision\":%.3f,\"recall\":%.3f,\"row\":[",(i>1?",":""),c,a[c]+0,p[c]+0,tp,pr,rc;
  for(j=1;j<=5;j++) printf "%s%d",(j>1?",":""),n[c"|"C[j]]+0; printf "]}" }
 printf "]}" }' > /tmp/conf.json
LOSS=$(paste $M/cb_cats/learn_error.tsv $M/cb_cats/test_error.tsv | awk -F"$TAB" 'NR==1{next} (NR-2)%50==0{printf "%s[%s,%.4f,%.4f]",(k++?",":""),$1,$2,$4}')
echo "{\"conf\":$(cat /tmp/conf.json),\"loss\":[$LOSS]}" > reports/web/mlcats.json
cat reports/ml/cats_run.txt
echo CATS_DONE
