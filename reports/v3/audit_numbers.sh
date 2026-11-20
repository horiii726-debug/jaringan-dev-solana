#!/usr/bin/env bash
# Stage 0 audit: reproduces every number in reports/v3/IMPLEMENTATION_AUDIT.md. Read-only on data, 0 provider credits.
# Usage: cd /root/sol && bash reports/v3/audit_numbers.sh > reports/v3/audit_numbers.txt
set -u
cd /root/sol
echo "## N1 git"
echo "head $(git rev-parse --short HEAD)"
echo "tracked_files $(git ls-files | wc -l)"
echo "untracked_or_modified $(git status --porcelain | wc -l)"
echo "## N2 data archive"
(cd data_gz && echo "md5_ok $(md5sum -c MD5SUMS 2>/dev/null | grep -c ': OK$') of $(wc -l < MD5SUMS)")
echo "archive_entries $(cat data_gz/data.tgz.p_* | tar -tz | wc -l)"
echo "data_dir_files $(find data -type f | wc -l)"
echo "## N3 datasets"
echo "ds_jsonl_rows $(wc -l < data/roots/ds.jsonl)"
echo "ds_jsonl_distinct_mints $(jq -r .mint data/roots/ds.jsonl | sort -u | wc -l)"
echo "ds_jsonl_keys $(head -1 data/roots/ds.jsonl | jq -c 'keys')"
echo "ds_jsonl_dex $(jq -r .dex data/roots/ds.jsonl | sort | uniq -c | sort -rn | awk '{printf "%s=%s ",$2,$1}')"
echo "mig_oct_rows $(wc -l < data/mig_oct.jsonl)"
echo "mig_new_rows $(wc -l < data/mig_new.jsonl)"
for f in data/launches_2026-11-1*.jsonl; do echo "launches $(basename $f) $(wc -l < $f)"; done
for f in dataset/launches/*.gz; do echo "launches_gz $(basename $f) $(zcat $f | wc -l)"; done
echo "## N4 infra labels (no source/date column)"
echo "exchange_list_p3 $(wc -l < data/p3/exchange_list.txt)"
echo "exchange_list_dataset $(wc -l < dataset/wallets/exchange_list.txt)"
echo "labels_tsv_rows $(wc -l < data/roots/labels.tsv)"
echo "labels_tsv_columns $(awk -F'\t' '{print NF}' data/roots/labels.tsv | sort -u | tr '\n' ' ')"
echo "labels_tsv_class $(cut -f2 data/roots/labels.tsv | sort | uniq -c | awk '{printf "%s=%s ",$2,$1}')"
(awk -F'\t' '$2=="EXCHANGE_SERVICE"||$2=="BOT_HUB"||$2=="KNOWN_LIST"{print $1}' data/roots/labels.tsv
 awk '{print $1}' data/p3/exchange_list.txt dataset/wallets/exchange_list.txt) | sort -u > /tmp/v3_infra.txt
echo "infra_union $(wc -l < /tmp/v3_infra.txt)"
echo "## N5 old family graph (data/family_a, family_export)"
E=data/family_a/edges.jsonl; W=data/family_a/wallets.jsonl; X=family_export/wallet_family.csv
echo "fa_edges $(wc -l < $E)"
echo "fa_edges_distinct_sig $(jq -r .sig $E | sort -u | wc -l)"
echo "fa_edges_sol_lt_0.1 $(jq -c 'select(.sol<0.1)' $E | wc -l)"
echo "fa_edges_sol_lt_1 $(jq -c 'select(.sol<1)' $E | wc -l)"
echo "fa_edges_endpoint_in_infra_union $( (jq -r .parent $E; jq -r .child $E) | grep -cFxf /tmp/v3_infra.txt)"
echo "fa_roots $(jq -r .root $E | sort -u | wc -l)"
echo "fa_top3_root_edges $(jq -r .root $E | sort | uniq -c | sort -rn | head -3 | awk '{printf "%s ",$1}')"
echo "fa_wallets $(wc -l < $W)"
echo "fa_wallets_busy $(jq -c 'select(.busy==true)' $W | wc -l)"
echo "fa_up_rows $(wc -l < data/family_a/up.jsonl)"
echo "fa_up_end $(jq -r .end data/family_a/up.jsonl | sed 's/:.*//' | sort | uniq -c | sort -rn | awk '{printf "%s=%s ",$2,$1}')"
echo "export_rows $(awk -F, 'NR>1' $X | awk 'END{print NR}')"
echo "export_roles $(awk -F, 'NR>1{print $3}' $X | sort | uniq -c | awk '{printf "%s=%s ",$2,$1}')"
echo "export_families $(awk -F, 'NR>1{print $1}' $X | sort -u | wc -l)"
echo "export_families_with_RAMAI $(awk -F, 'NR>1&&$3=="RAMAI"{print $1}' $X | sort -u | wc -l)"
echo "export_RAMAI_ntx_min_max $(awk -F, 'NR>1&&$3=="RAMAI"{print $5}' $X | sort -n | sed -n '1p;$p' | tr '\n' ' ')"
echo "export_family_size $(awk -F, 'NR>1{n[$1]++} END{for(k in n) print n[k]}' $X | sort -n | awk '{a[NR]=$1} END{printf "n=%d min=%d med=%d p90=%d max=%d", NR, a[1], a[int((NR+1)/2)], a[int(NR*0.9)], a[NR]}')"
echo
echo "export_ujung_atas $(awk -F, 'NR>1{print $4}' family_export/family_ringkas.csv | sed 's/:.*//' | sort | uniq -c | awk '{printf "%s=%s ",$2,$1}')"
echo "## N6 MadeOnSol probe (raw files saved 2026-11-20, 3 calls)"
M=data/v3/madeonsol
echo "openapi_version $(jq -r .info.version $M/openapi.json) paths $(jq -r '.paths|keys[]' $M/openapi.json | wc -l)"
echo "tier $(jq -r .tier $M/probe_me.json) daily_limit $(jq -r .quota.daily.limit $M/probe_me.json)"
for p in /wallet/{address}/funding /wallet/{address}/flags /wallet/batch/classify /deployer-hunter/{wallet} /deployer-hunter/{wallet}/as-of /token/{mint} /tokens/{mint}/bundles /tokens/{mint}/peak-history; do
  echo "gate $p $(jq -r --arg p "$p" '.paths[$p]|to_entries[0].value["x-madeonsol-tier"]' $M/openapi.json)"; done
echo "token_3MG_deployer $(jq -r .token.deployer.wallet $M/probe_token_3MG.json)"
echo "our_3MG_creator $(grep CZiKhMmpsSC3zxTYxZufPHGCoRV8S1KZyBqEQ4oJpump family_export/koin_family.csv | cut -d, -f4)"
echo "## N7 provider keys present (names only)"
for f in .env .rpc.env .bitquery_token .madeonsol.env; do [ -f "$f" ] && echo "$f present keys=$(cut -d= -f1 "$f" | tr '\n' ' ')" || echo "$f MISSING"; done
echo "## N8 hashes"
sha256sum data/roots/ds.jsonl data/family_a/edges.jsonl family_export/wallet_family.csv | awk '{print substr($1,1,16), $2}'
echo "## N9 round-2 additions (CRITIC T0_r1 findings)"
echo "state_md_lines $(wc -l < checkpoints/STATE.md)"
echo "archive_dirs $(cat data_gz/data.tgz.p_* | tar -tzv | grep -c '^d') archive_files $(cat data_gz/data.tgz.p_* | tar -tzv | grep -c '^-')"
echo "data_v3_files $(find data/v3 -type f | wc -l)"
echo "archive_uncompressed_bytes $(cat data_gz/data.tgz.p_* | tar -tzv | awk '{s+=$3} END{print s}')"
echo "dataset_du $(du -sh dataset | cut -f1)"
WSOL=So11111111111111111111111111111111111111112
for f in data/mig_oct.jsonl data/raw/migrations.jsonl data/mig_new.jsonl; do
  echo "src $f rows=$(awk 'END{print NR}' $f) mints=$(jq -r .mint $f | sort -u | wc -l) wsol_mints=$(jq -r --arg w $WSOL 'select(.quote==$w)|.mint' $f | sort -u | wc -l)"; done
cat data/mig_oct.jsonl data/raw/migrations.jsonl data/mig_new.jsonl | jq -r .mint | sort -u > /tmp/v3_all.txt
cat data/mig_oct.jsonl data/raw/migrations.jsonl data/mig_new.jsonl | jq -r --arg w $WSOL 'select(.quote==$w)|.mint' | sort -u > /tmp/v3_wsol.txt
echo "union_all_mints $(wc -l < /tmp/v3_all.txt) union_wsol_mints $(wc -l < /tmp/v3_wsol.txt)"
jq -r .mint data/roots/ds.jsonl | sort -u > /tmp/v3_ds.txt
echo "ds_mints_in_union_wsol $(comm -12 /tmp/v3_ds.txt /tmp/v3_wsol.txt | wc -l) ds_mints_in_union_all $(comm -12 /tmp/v3_ds.txt /tmp/v3_all.txt | wc -l)"
echo "roots_sh_mcap_fallback $(grep -n -o 'mcap: (.marketCap // .fdv)' roots.sh)"
N=$(wc -l < /tmp/v3_all.txt); C=$(( (N+29)/30 )); echo "ds_calls_for_union_all $C minutes_at_300_per_min $(( (C+299)/300 ))"
echo "madeonsol_token_lookups_per_day_max $(jq -r .quota.daily.limit data/v3/madeonsol/probe_me.json)"
echo "ds_now_mcap_ge_50k $(jq 'select(.mcap>=50000)' -c data/roots/ds.jsonl | wc -l) ge_100k $(jq 'select(.mcap>=100000)' -c data/roots/ds.jsonl | wc -l) ge_1m $(jq 'select(.mcap>=1000000)' -c data/roots/ds.jsonl | wc -l) (current snapshot, mcap may be FDV fallback)"
X=family_export/wallet_family.csv; E=data/family_a/edges.jsonl
echo "export_rows_robust $(awk -F, 'NR>1' $X | awk 'END{print NR}') export_role_sum $(awk -F, 'NR>1{n++} END{print n}' $X)"
echo "export_wallets_in_gt1_family $(awk -F, 'NR>1{print $2"\t"$1}' $X | sort -u | cut -f1 | uniq -d | wc -l)"
echo "fa_sigs_with_gt1_edge $(jq -r .sig $E | sort | uniq -d | wc -l) fa_max_edges_per_sig $(jq -r .sig $E | sort | uniq -c | sort -rn | head -1 | awk '{print $1}')"
echo "fa_children_with_gt1_parent $(jq -r '[.child,.parent]|@tsv' $E | sort -u | cut -f1 | uniq -d | wc -l)"
echo "fa_wallets_roots $(jq -r .root data/family_a/wallets.jsonl | sort -u | wc -l) export_families $(awk -F, 'NR>1{print $1}' $X | sort -u | wc -l)"
echo "export_families_not_edge_roots $(comm -23 <(awk -F, 'NR>1{print $1}' $X | sort -u) <(jq -r .root $E | sort -u) | wc -l)"
