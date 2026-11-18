#!/bin/sh
# Self-learning loop: every 3 h retrain on all data downloaded so far, log one line per run.
cd /root/jaringan
while true; do
  for d in 2 0o j1; do
    JAR_TIP_FRAC=$( [ $d = 2 ] && echo 0 || echo 0.0146 ) ./ml.sh $d > reports/ml/run_d$d.txt 2>&1
    echo "$(date -u +%FT%TZ) d=$d $(grep "^rows train" reports/ml/run_d$d.txt) | $(grep "^test:" reports/ml/run_d$d.txt | head -1) | $(grep "top 10%" reports/ml/run_d$d.txt) | shuffle: $(grep "^test:" reports/ml/run_d$d.txt | tail -1 | grep -o "AUC=[0-9.]*")" >> reports/ml/retrain_log.md
  done
  sleep 10800
done
