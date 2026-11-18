#!/bin/bash
# Bubblemap-style holder clusters for one mint (owner 18 Nov):
#   all holders (Helius DAS getTokenAccounts, 1000 per page) -> top N by balance -> first funder of each holder
#   (strict: direct transfer) -> group holders by funder; funders labelled (exchange / service / bot never make a family).
# Output: data/holders/<mint>/clusters.tsv  (funder, label, holders, % supply)
cd /root/jaringan || exit 1
. ./.env
M=$1; N=${2:-300}; D=data/holders/$M; mkdir -p $D
: > $D/holders.tsv
for p in $(seq 1 20); do
  curl -s -m 60 "https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY" -H "content-type: application/json" \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTokenAccounts\",\"params\":{\"mint\":\"$M\",\"limit\":1000,\"page\":$p}}" > $D/p.json
  n=$(jq '.result.token_accounts|length' $D/p.json 2>/dev/null); [ "${n:-0}" = 0 ] && break
  jq -r '.result.token_accounts[] | select(.amount>0) | [.owner, .amount] | @tsv' $D/p.json >> $D/holders.tsv
  [ "$n" -lt 1000 ] && break
done
sort -t$'\t' -k2,2nr $D/holders.tsv > $D/h_sorted.tsv
echo "holders=$(wc -l < $D/holders.tsv)"
head -$N $D/h_sorted.tsv | cut -f1 > $D/top.txt
./target/release/jaringan fund $D/top.txt $D/fund.jsonl > /dev/null 2>&1
jq -r 'select(.funder!=null and .kind=="transfer")|.funder' $D/fund.jsonl | sort | uniq -c | awk '$1>=2{print $2}' > $D/f2.txt
./target/release/jaringan svc $D/f2.txt $D/svc.jsonl > /dev/null 2>&1
jq -r '[.w,.label,.n_sigs]|@tsv' $D/svc.jsonl > $D/lab.tsv
jq -r 'select(.funder!=null)|[.w,.funder,.kind,(.lam//0)]|@tsv' $D/fund.jsonl > $D/ff.tsv
gawk -F'\t' 'ARGIND==1{lab[$1]=$2; ns[$1]=$3; next} ARGIND==2{amt[$1]=$2; next}
  ARGIND==3 && $3=="transfer" { f=$2; n[f]++; s[f]+=amt[$1] }
  END { for (f in n) if (n[f]>=2) { l=(f in lab)?lab[f]:"?"; if (l=="OPERATOR" && ns[f]>=1000) l="OPERATOR_ramai"; printf "%s\t%s\t%d\t%.2f\n", f, l, n[f], 100*s[f]/1e15 } }' \
  $D/lab.tsv $D/h_sorted.tsv $D/ff.tsv | sort -t$'\t' -k4,4nr > $D/clusters.tsv
echo "top=$N holders traced; clusters (funder label holders %supply):"
head -15 $D/clusters.tsv
awk -F'\t' '$2=="OPERATOR"{h+=$3; s+=$4; n++} END{printf "FAMILY clusters (non-exchange, non-bot): %d, holders %d, supply %.2f%%\n", n, h, s}' $D/clusters.tsv
