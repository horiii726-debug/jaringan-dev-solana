#!/bin/bash
# ML on ALL creates: will this coin hold a real market cap >= $50k? Only facts known at the create tx.
# Time split: train 6-12 Nov, valid 13 Nov, test 14-16 Nov (1 day gap is covered by the 24 h settle rule).
cd /root/jaringan || exit 1
CB=/root/bin/catboost; M=data/ml; TAB=$(printf '\t')
L=$(ls data/launches_2026-1*.jsonl | paste -sd,)
F=$(ls data/p3/fund_*.jsonl data/p3/fund_*.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | paste -sd,)
./target/release/jaringan feat_all $L $F data/p2amm/min50k_fix2.tsv $M/fall.tsv
H=$(head -1 $M/fall.tsv)
awk -F"$TAB" 'NR==1 || ($2>="2026-11-06" && $2<="2026-11-12")' $M/fall.tsv > $M/btrain.tsv
awk -F"$TAB" 'NR==1 || $2=="2026-11-13"' $M/fall.tsv > $M/bvalid.tsv
awk -F"$TAB" 'NR==1 || $2>="2026-11-14"' $M/fall.tsv > $M/btest.tsv
for s in btrain bvalid btest; do echo "$s rows=$(($(wc -l < $M/$s.tsv)-1)) positives=$(awk -F"$TAB" 'NR>1 && $1==1' $M/$s.tsv | wc -l)"; done
mkdir -p $M/cb_big
$CB fit --learn-set $M/btrain.tsv --test-set $M/bvalid.tsv --cd $M/fall.tsv.cd --has-header --loss-function Logloss --eval-metric PRAUC \
  --custom-metric AUC --auto-class-weights SqrtBalanced --iterations 3000 --learning-rate 0.05 --depth 6 --od-type Iter --od-wait 300 \
  --use-best-model --task-type GPU --devices 0 --random-seed 42 --train-dir $M/cb_big -m model.cbm --logging-level Silent > $M/cb_big.log 2>&1 || { tail -3 $M/cb_big.log; exit 1; }
$CB calc -m $M/cb_big/model.cbm --input-path $M/btest.tsv --cd $M/fall.tsv.cd --has-header --output-path $M/bpred.tsv --prediction-type Probability > /dev/null 2>&1
$CB eval-metrics -m $M/cb_big/model.cbm --input-path $M/btest.tsv --cd $M/fall.tsv.cd --has-header --metrics "Logloss,AUC,PRAUC" --result-dir $M/cb_big_eval > /dev/null 2>&1
echo "== loss (valid) start / best: $(awk -F"$TAB" 'NR==1{for(i=1;i<=NF;i++) if($i=="Logloss") c=i; next} NR==2{s=$c; b=$c; it=0} $c<b{b=$c; it=$1} END{printf "%.4f / %.4f (iter %s)", s, b, it}' $M/cb_big/test_error.tsv)"
echo "== test metrics (last row = final model): $(tail -1 $M/cb_big_eval/eval_result.tsv 2>/dev/null)"
echo "== test: hit rate of the top-k coins by model score (label = held >= \$50k)"
paste <(tail -n +2 $M/btest.tsv | cut -f1) <(tail -n +2 $M/bpred.tsv | awk -F"$TAB" '{print $NF}') | sort -t"$TAB" -k2,2gr | \
  awk -F"$TAB" '{n++; p+=$1; if(n==50||n==100||n==500||n==1000||n==5000) printf "  top %5d: %4d hit (%.1f%%)\n", n, p, 100*p/n; tot++; P+=$1} END{printf "  base: %d of %d (%.2f%%)\n", P, tot, 100*P/tot}'
echo "== feature importance"
$CB fstr -m $M/cb_big/model.cbm --input-path $M/btrain.tsv --cd $M/fall.tsv.cd --has-header -o $M/bfstr.tsv > /dev/null 2>&1; sort -t"$TAB" -k1,1nr $M/bfstr.tsv | head -10 | tr "$TAB" " " | paste -sd";"
echo MLBIG_DONE
