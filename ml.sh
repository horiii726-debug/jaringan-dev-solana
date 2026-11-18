#!/bin/bash
# Labels + as-of features from all trade data so far, CatBoost train/valid/test, evaluation, label-shuffle attack.
# Split by time: train = day 14 (first 80%), valid = day 14 (last 20%), test = day 15; 30 min purge before test.
# Usage: ml.sh <delay>   (2 = 2 slots after create; 0o = first in the create block, "Jito" case)
cd /root/jaringan || exit 1
D=${1:-2}; CB=/root/bin/catboost; M=data/ml; TAB=$(printf '\t')
L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
T=$(ls data/p2/tr_rand_a.jsonl data/p2/tr_rand_b.part_*.jsonl data/p2/tr_all_a.jsonl data/p2/tr_all_b.part_*.jsonl 2>/dev/null | paste -sd,)
F=$(ls data/p3/fund_*.jsonl data/p3/fund_d1.part_*.jsonl data/p3/fund_d2.part_*.jsonl 2>/dev/null | paste -sd,)
./target/release/jaringan outcomes $M/outcomes.csv data/launches_2026-11-14.jsonl,data/launches_2026-11-15.jsonl $T
./target/release/jaringan feat $L $T $F $M/outcomes.csv $D $M/feat_d$D.tsv
H=$(head -1 $M/feat_d$D.tsv)
T0=$(awk -F"$TAB" 'NR>1 && $4=="2026-11-15"{print $5}' $M/feat_d$D.tsv | sort -n | head -1)
awk -F"$TAB" -v t="$T0" 'NR>1 && $4=="2026-11-14" && $5+1800<=t' $M/feat_d$D.tsv | sort -t"$TAB" -k5,5n > /tmp/trall.tsv
N=$(wc -l < /tmp/trall.tsv); NV=$((N/5))
{ echo "$H"; head -n $((N-NV)) /tmp/trall.tsv; } > $M/train.tsv
{ echo "$H"; tail -n $NV /tmp/trall.tsv; } > $M/valid.tsv
awk -F"$TAB" 'NR==1 || $4=="2026-11-15"' $M/feat_d$D.tsv > $M/test.tsv
echo "rows train=$((N-NV)) valid=$NV test=$(($(wc -l < $M/test.tsv)-1))"
fit() { # $1 learn, $2 dir, $3 seed, $4 valid set
  $CB fit --learn-set "$1" --test-set "${4:-$M/valid.tsv}" --cd $M/feat_d$D.tsv.cd --has-header --loss-function Logloss --eval-metric AUC \
    --custom-metric Logloss --iterations 3000 --learning-rate 0.03 --depth 6 --l2-leaf-reg 5 --od-type Iter --od-wait 300 \
    --use-best-model --task-type GPU --devices 0 --random-seed "${3:-42}" --train-dir "$2" -m model.cbm --logging-level Silent > "$2.log" 2>&1
}
calc() { $CB calc -m "$1" --input-path "$2" --cd $M/feat_d$D.tsv.cd --has-header --output-path "$3" --prediction-type Probability > /dev/null 2>&1; }
mkdir -p $M/cb_d$D $M/cb_shuf
fit $M/train.tsv $M/cb_d$D 42 || { tail -3 $M/cb_d$D.log; exit 1; }
calc $M/cb_d$D/model.cbm $M/valid.tsv $M/pred_valid.tsv
calc $M/cb_d$D/model.cbm $M/test.tsv $M/pred_test.tsv
echo "== loss curve: iter | train Logloss | valid Logloss | valid AUC"
paste $M/cb_d$D/learn_error.tsv $M/cb_d$D/test_error.tsv | awk -F"$TAB" 'NR==1{for(i=1;i<=NF;i++) if($i=="Logloss") c[++k]=i; for(i=1;i<=NF;i++) if($i=="AUC") a=i; next} (NR-2)%100==0{printf "%s | %.4f | %.4f | %.4f\n",$1,$c[1],$c[2],$a}' | head -16
echo "best iteration (valid AUC): $(awk -F"$TAB" 'NR==1{for(i=1;i<=NF;i++) if($i=="AUC") a=i; next} $a>b{b=$a; it=$1} END{print it, b}' $M/cb_d$D/test_error.tsv)"
./target/release/jaringan mleval $M/valid.tsv $M/pred_valid.tsv $M/test.tsv $M/pred_test.tsv
echo "== feature importance (top 12)"
$CB fstr -m $M/cb_d$D/model.cbm --input-path $M/train.tsv --cd $M/feat_d$D.tsv.cd --has-header -o $M/fstr.tsv > /dev/null 2>&1
sort -t"$TAB" -k1,1nr $M/fstr.tsv | head -12 | tr "$TAB" " " | paste -sd";"
echo "== attack: shuffled train labels (test AUC must be ~0.5)"
{ echo "$H"; tail -n +2 $M/train.tsv | cut -f1 | shuf --random-source=<(yes 7) > /tmp/lab.txt; tail -n +2 $M/train.tsv | cut -f2- | paste /tmp/lab.txt -; } > $M/train_shuf.tsv
{ echo "$H"; tail -n +2 $M/valid.tsv | cut -f1 | shuf --random-source=<(yes 9) > /tmp/lv.txt; tail -n +2 $M/valid.tsv | cut -f2- | paste /tmp/lv.txt -; } > $M/valid_shuf.tsv
fit $M/train_shuf.tsv $M/cb_shuf 42 $M/valid_shuf.tsv && calc $M/cb_shuf/model.cbm $M/test.tsv $M/pred_shuf.tsv && \
  ./target/release/jaringan mleval $M/valid.tsv $M/pred_valid.tsv $M/test.tsv $M/pred_shuf.tsv | grep "^test:"
echo ML_DONE
