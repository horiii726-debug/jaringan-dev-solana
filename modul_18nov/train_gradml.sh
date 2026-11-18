#!/bin/bash
# ML "lulus atau mati di Pump.fun" (18 Nov). Persis perintah yang dipakai di VPS.
# 1) fitur (Rust)  2) latih CatBoost GPU (validasi = 20% terakhir data latih, urut waktu)
# 3) prediksi minggu uji  4) evaluasi (Rust)  5) pentingnya fitur
# Hasil 18 Nov: AUC uji 0,80 | Logloss 0,0863 vs tebak-rata-rata 0,0915 | top 0,1% presisi 90%.
set -e
cd /root/jaringan
CB=/root/bin/catboost          # CatBoost CLI 1.2.7 (GPU)
D=data/gml

./target/release/jaringan gradml build $D        # -> train.tsv (7-12 Nov), test.tsv (13-16 Nov), pool.cd

cd $D
N=$(($(wc -l < train.tsv)-1)); NV=$((N/5))
{ head -1 train.tsv; tail -n +2 train.tsv | head -n $((N-NV)); } > tr.tsv
{ head -1 train.tsv; tail -n $NV train.tsv; } > va.tsv

$CB fit --learn-set tr.tsv --test-set va.tsv --cd pool.cd --has-header \
  --loss-function Logloss --eval-metric AUC --custom-metric Logloss \
  --iterations 3000 --learning-rate 0.03 --depth 6 --od-type Iter --od-wait 300 --use-best-model \
  --task-type GPU --devices 0 --random-seed 42 --train-dir cb -m model.cbm --logging-level Silent

$CB calc -m cb/model.cbm --input-path test.tsv --cd pool.cd --has-header \
  --output-path pred.tsv --prediction-type Probability

cd /root/jaringan
./target/release/jaringan gradml eval $D/test.tsv $D/pred.tsv
$CB fstr -m $D/cb/model.cbm --input-path $D/tr.tsv --cd $D/pool.cd --has-header -o $D/fstr.tsv
head -10 $D/fstr.tsv

# Langkah berikut (belum selesai 18 Nov): ambil 2% skor tertinggi minggu uji, tarik transaksi curve-nya,
# lalu simulasi untung beli < 1 detik dengan entry1s:
#   ./target/release/jaringan trades data/gml/top2.csv data/gml/top2_trades.jsonl
#   ./target/release/jaringan entry1s data/gml/top2_c.tsv data/gml/top2_trades.jsonl -
