#!/bin/bash
# ML v3 training (18 Nov). CatBoost MultiClass on GPU; valid = 14 Nov 20-24 h (early stopping + rule choice), test = 15 Nov (once).
set -e
cd /root/jaringan/data/ml3
CB=/root/bin/catboost
$CB fit --learn-set train.tsv --test-set valid.tsv --cd pool.cd --has-header --loss-function MultiClass --eval-metric MultiClass \
  --iterations 3000 --learning-rate 0.03 --depth 6 --od-type Iter --od-wait 300 --use-best-model \
  --task-type GPU --devices 0 --random-seed 42 --train-dir cb -m model.cbm --logging-level Silent > cb.log 2>&1
echo "best iteration: $(tail -n +2 cb/test_error.tsv | sort -t$'\t' -k2,2g | head -1 | cut -f1) / $(tail -1 cb/test_error.tsv | cut -f1)"
for s in valid test; do $CB calc -m cb/model.cbm --input-path $s.tsv --cd pool.cd --has-header --output-path pred_$s.tsv --prediction-type Probability > /dev/null 2>&1; done
$CB fstr -m cb/model.cbm --input-path train.tsv --cd pool.cd --has-header -o fstr.tsv > /dev/null 2>&1 || true
# shuffle check: labels permuted in train -> test loss must be ~ baseline (no leak through the pipeline)
paste <(cut -f1 train.tsv | tail -n +2 | shuf --random-source=<(yes)) <(cut -f2- train.tsv | tail -n +2) > tr_shuf.body
{ head -1 train.tsv; cat tr_shuf.body; } > train_shuf.tsv; rm -f tr_shuf.body
$CB fit --learn-set train_shuf.tsv --test-set valid.tsv --cd pool.cd --has-header --loss-function MultiClass --iterations 3000 \
  --learning-rate 0.03 --depth 6 --od-type Iter --od-wait 300 --use-best-model --task-type GPU --devices 0 --random-seed 42 \
  --train-dir cb_shuf -m model_shuf.cbm --logging-level Silent > cb_shuf.log 2>&1
$CB calc -m cb_shuf/model_shuf.cbm --input-path test.tsv --cd pool.cd --has-header --output-path pred_test_shuf.tsv --prediction-type Probability > /dev/null 2>&1
cd /root/jaringan
./target/release/jaringan ml3 eval data/ml3 data/ml3/fam_known.txt > /dev/null && cp data/ml3/report.txt data/ml3/report_family_dikenal.txt
./target/release/jaringan ml3 eval data/ml3 data/ml3/fam_all.txt > /dev/null && cp data/ml3/report.txt data/ml3/report_family_semua.txt
# shuffled-model test loss (same eval code, predictions swapped in a copy)
mkdir -p data/ml3_shuf && cp data/ml3/rows.tsv data/ml3/pred_valid.tsv data/ml3_shuf/ && cp data/ml3/pred_test_shuf.tsv data/ml3_shuf/pred_test.tsv
./target/release/jaringan ml3 eval data/ml3_shuf > /dev/null 2>&1 || true
head -1 data/ml3_shuf/report.txt > data/ml3/shuffle_check.txt || true
echo ML3_TRAIN_DONE
