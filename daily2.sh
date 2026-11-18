#!/bin/bash
# Self-learning loop (VPS, hourly check): for each new UTC day pull creates, graduations and PumpSwap peaks,
# curve trades only for coins that matter (graduates + coins of watched wallets), trace their devs' funders via
# Shyft, then rebuild labels, the $50k model (GPU), the A/B/C classes and the watchlist. The monitor reloads the
# watchlist file on change, so open paper positions are kept.
cd /root/jaringan
set -a; . ./.env; set +a
while true; do
  D=$(date -u -d "yesterday" +%F); S=$(date -u -d "$D" +%s); E=$((S+86400))
  if ! grep -q "\"done\":true" data/launches_$D.jsonl.ckpt 2>/dev/null; then
    ./target/release/jaringan launches $S $E data/launches_$D.jsonl > logs/launches_$D.log 2>&1
    ./target/release/jaringan migrations $S $E data/p2amm/mig_$D.jsonl > logs/mig_$D.log 2>&1
    cat data/p2amm/mig_$D.jsonl >> data/p2amm/migrations.jsonl
    sort -u -o data/p2amm/migrations.jsonl data/p2amm/migrations.jsonl  # F10: rerun must not duplicate
    jq -r 'select(.quote=="So11111111111111111111111111111111111111112")|[.mint, ("pool="+.pool), .slot, .bt]|@csv' data/p2amm/mig_$D.jsonl | tr -d '"' > data/p2amm/mig_list_$D.txt
    JAR_AMM_MAX_PAGES=1 ./target/release/jaringan amm data/p2amm/mig_list_$D.txt data/p2amm/amm_$D.jsonl > logs/amm_$D.log 2>&1
    cat data/p2amm/amm_$D.jsonl >> data/p2amm/amm_all.jsonl
    sort -u -o data/p2amm/amm_all.jsonl data/p2amm/amm_all.jsonl  # F10: rerun must not duplicate
    jq -r '.teams[].devs[].w, .teams[].watch[]' reports/web/watch_all.json | sort -u > /tmp/wl.txt
    jq -r 'select(.kind=="create")|[.mint,.curve,.bt,.creator]|@csv' data/launches_$D.jsonl | tr -d '"' | gawk -F, 'ARGIND==1{w[$1]=1;next} ARGIND==2{g[$1]=1;next} ($4 in w)||($1 in g){print $1","$2","$3}' /tmp/wl.txt <(cut -d, -f1 data/p2amm/mig_list_$D.txt) - > data/p2/need_$D.csv
    ./target/release/jaringan trades data/p2/need_$D.csv data/p2/tr_$D.jsonl > logs/tr_$D.log 2>&1
    jq -r 'select(.kind=="create")|.creator' data/launches_$D.jsonl | gawk -F, 'ARGIND==1{g[$1]=1;next} 1' <(cut -d, -f1 data/p2amm/mig_list_$D.txt) - | sort -u > /tmp/newdevs.txt
    JAR_RPC_URL="https://rpc.shyft.to?api_key=$SHYFT_KEY" ./chain.sh /tmp/newdevs.txt d$D > logs/chain_$D.log 2>&1
    NEWDAY=1
  fi
  # rebuild labels, model, classes, watchlist
  jq -r 'select(.t)|[.mint, ([.t[]|select(.[6]>=20000000000 and .[5]>=50000000000000 and (.[6]/.[5]*1000000*71.08)>=50000)]|length), ([.t[]|select(.[6]>=20000000000 and .[5]>=50000000000000)|(.[6]/.[5]*1000000*71.08)]|max // 0)]|@tsv' data/p2amm/amm_all.jsonl | awk -F"\t" '$2>=20{print $1"\t"$2"\t"$3}' > data/p2amm/min50k_fix2.tsv
  ./ml_big.sh > reports/ml/ml_big.txt 2>&1
  L=$(ls data/launches_2026-1*.jsonl | paste -sd,); T=$(ls data/p2/tr_*.jsonl data/p2/tr_*.part_*.jsonl 2>/dev/null | paste -sd,)
  ./target/release/jaringan cats $L $T data/ml/cats_all.tsv > /dev/null 2>&1
  jq -r 'select(.kind=="create")|[.mint,.creator]|@tsv' data/launches_2026-1*.jsonl > /tmp/mc.tsv
  (printf "kelas\tjenis\twallet\tkoin_total\tkoin_ada_data\tkoin_min50k\tkoin_naik\tragpul\tsepi\n"; gawk -f kelas.awk data/ml/cats_all.tsv data/p2amm/min50k_fix2.tsv reports/team_members.tsv /tmp/mc.tsv | sort -t"	" -k1,1 -k6,6nr) > reports/kelas_abc.tsv
  echo "$(date -u +%FT%TZ) day=$D $(grep -E "top +50:|base:" reports/ml/ml_big.txt | tr '\n' ' ') kelas: $(tail -n +2 reports/kelas_abc.tsv | cut -f1,2 | sort | uniq -c | tr '\n' ' ')" >> reports/daily2_log.md
  # back up the new day's data to GitHub (secret-checked: abort if any .env value is in a tracked file)
  if [ "${NEWDAY:-0}" = 1 ]; then
    ./gh_prepare.sh > logs/gh_prepare.log 2>&1
    git add -A
    ok=1
    for v in $(grep -E "^[A-Z_]+=" .env | cut -d= -f2-); do git grep -q --cached -F -- "$v" && ok=0; done
    if [ $ok = 1 ]; then
      git commit -q -m "Daily data $D (auto)"
      printf '#!/bin/sh\necho "$GITHUB_TOKEN"\n' > /tmp/askpass.sh; chmod 700 /tmp/askpass.sh
      GIT_ASKPASS=/tmp/askpass.sh GIT_TERMINAL_PROMPT=0 git push -q origin main >> logs/gh_auto.log 2>&1
      rm -f /tmp/askpass.sh
      echo "$(date -u +%FT%TZ) pushed $D" >> logs/gh_auto.log
    else
      git reset -q; echo "$(date -u +%FT%TZ) SECRET_ABORT" >> logs/gh_auto.log
    fi
    NEWDAY=0
  fi
  sleep 3600
done
