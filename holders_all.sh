#!/bin/bash
# Bubblemap-style holder clusters for every coin we work on: JALU, GATO, then the 110 DexScreener winners
# (data/roots/surv_mints.txt). Resumable (skips coins with clusters.tsv). Stops cleanly at the Helius credit cap.
# Summary: data/holders/SUMMARY.tsv  mint sym holders family_clusters family_holders family_supply% top_family_funder
cd /root/jaringan || exit 1
LIST=data/holders/all_mints.txt; mkdir -p data/holders
{ echo BSsmLA6UN8fzJ8qUkV4KwC5LxR7ab3TaUtcwvAyJpump; echo HsprHxBRgiVaQdRf3TnKNuZZtaX9JUSPgMFdAkk6pump; cat data/roots/surv_mints.txt; } | awk '!s[$0]++' > $LIST
echo "coins=$(wc -l < $LIST) start=$(date -u +%FT%TZ)"
while read -r M; do
  [ -s data/holders/$M/clusters.tsv ] && continue
  ./holders.sh $M 200 > data/holders/$M.log 2>&1
  if grep -q BUDGET_CAP data/holders/$M.log 2>/dev/null || grep -rq BUDGET_CAP data/holders/$M/ 2>/dev/null; then echo "budget cap reached at $M"; break; fi
done < $LIST
echo -e "mint\tsym\tholders\tfamily_clusters\tfamily_holders\tfamily_supply_pct\ttop_family_funder" > data/holders/SUMMARY.tsv
while read -r M; do
  D=data/holders/$M; [ -s $D/clusters.tsv ] || continue
  sym=$(grep -h "\"$M\"" data/roots/ds.jsonl data/moonall/moon.jsonl 2>/dev/null | head -1 | jq -r '.sym // "?"' 2>/dev/null)
  h=$(wc -l < $D/holders.tsv)
  awk -F'\t' -v m=$M -v s="${sym:-?}" -v h=$h '$2=="OPERATOR"{n++; hh+=$3; p+=$4; if($4>bp){bp=$4; bf=$1}} END{printf "%s\t%s\t%d\t%d\t%d\t%.2f\t%s\n", m, s, h, n, hh, p, bf}' $D/clusters.tsv >> data/holders/SUMMARY.tsv
done < $LIST
for f in checkpoints/credits/*.txt; do cat $f; echo; done | awk '{s+=$1} END{print "credits_ledger_total=" s}'
echo HOLDERS_ALL_DONE $(date -u +%FT%TZ)
