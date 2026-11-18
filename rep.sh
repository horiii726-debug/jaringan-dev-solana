#!/bin/bash
# Reputation from the FULL history of every A/B/C wallet + post-graduation pool check + origin + new linked
# wallets. Bad wallets/teams are removed from the watchlist (monitor hot-reloads it). Rules v1 (17 Nov):
#  rug-like coin = the wallet sold its own coin within 60 s of creation
#  bad wallet    = rug-like >= 3, or rug-like share >= 30% (>= 2 coins), or >= 50% of its graduated coins drained
#                  (pool < 5 SOL now), or >= 50% quiet coins (>= 3 coins with trade data)
#  bad team      = >= 30% of its wallets with coins are bad
cd /root/jaringan
H=data/hist/history.jsonl; R=reports/reputasi; mkdir -p $R
jq -r '.w as $w | .coins[] | [$w, .mint, (.first_sell_s // -1), .sell_sol, .buy_sol]|@tsv' $H > $R/coins_alltime.tsv
# drained pools (class A coins checked on-chain), quiet from trade categories
gawk -F'\t' -v OFS='\t' '
  ARGIND==1 { drain[$1] = ($2 < 5); seen[$1] = 1; next }
  ARGIND==2 { cat[$1] = $2; next }
  { w=$1; m=$2; n[w]++; if ($3 >= 0 && $3 <= 60) rug[w]++
    if (m in seen) { g[w]++; if (drain[m]) dr[w]++ }
    if (m in cat) { k[w]++; if (cat[m]=="SEPI") q[w]++ } }
  END { for (w in n) { r=rug[w]+0; bad = (r>=3) || (n[w]>=2 && r>=0.3*n[w]) || (g[w]>0 && dr[w]>=0.5*g[w]) || (k[w]>=3 && q[w]>=0.5*k[w])
        print w, n[w], r, g[w]+0, dr[w]+0, k[w]+0, q[w]+0, (bad?"BAD":"OK") } }' /tmp/anow_mc.tsv data/ml/cats_all.tsv $R/coins_alltime.tsv > $R/wallet_rep.tsv
# origin: walk funder chain (all fund rows incl. Shyft) until exchange / unknown
cat data/p3/fund_*.jsonl data/p3/fund_*.part_*.jsonl data/p4/fund_*.jsonl data/p4/fund_*.part_*.jsonl data/p4/chain_*.jsonl 2>/dev/null | jq -r 'select(.funder!=null)|[.w,.funder]|@tsv' | sort -u -k1,1 > /tmp/wf_all.tsv
gawk -F'\t' -v OFS='\t' 'ARGIND==1{ex[$1]=1;next} ARGIND==2{f[$1]=$2;next}
  { w=$1; r=w; lv=0; why="akar"; while (lv<8) { if(!(r in f)){why=(lv==0?"tidak_diketahui":"akar");break} nx=f[r]; if(nx in ex){why="dana_dari_exchange";break} r=nx; lv++ } print w, r, lv, why }' \
  data/p3/exchange_list.txt /tmp/wf_all.tsv data/hist/wallets.txt > $R/origin.tsv
# new linked wallets from Bitquery transfers (>= 0.05 SOL, last day)
jq -r '[.parent,.child]|@tsv' data/hist/bq_links.jsonl 2>/dev/null | sort -u > $R/links_bitquery.tsv
# rebuild the watchlist without bad wallets / teams, adding linked wallets to their team
gawk -F'\t' 'ARGIND==1{st[$1]=$8; next} ARGIND==2{lk[$1]=lk[$1]" "$2; next}' $R/wallet_rep.tsv $R/links_bitquery.tsv /dev/null
jq --slurpfile rep <(gawk -F'\t' '{printf "{\"w\":\"%s\",\"s\":\"%s\"}\n", $1, $8}' $R/wallet_rep.tsv) \
   --slurpfile lk <(gawk -F'\t' '{printf "{\"p\":\"%s\",\"c\":\"%s\"}\n", $1, $2}' $R/links_bitquery.tsv) '
  ($rep | map({key:.w, value:.s}) | from_entries) as $S |
  ($lk | group_by(.p) | map({key:.[0].p, value:(map(.c))}) | from_entries) as $L |
  .teams |= map(
     . as $t |
     ([$t.devs[].w] | map(select($S[.] != null))) as $withc |
     ([$withc[] | select($S[.]=="BAD")] | length) as $nb |
     . + {bad_wallets:$nb, rated_wallets:($withc|length),
          devs: [$t.devs[] | select($S[.w] != "BAD")],
          watch: ((($t.watch // []) + ([$t.devs[].w, $t.root] | map($L[.] // []) | add // [])) | unique)}
  ) | .teams |= map(select(.rated_wallets == 0 or (.bad_wallets < 0.3 * .rated_wallets)))
  | .teams |= map(select((.devs|length) > 0))' reports/web/watch_all.json > $R/watch_clean.json
echo "== wallet: $(awk -F'\t' '{n++; if($8=="BAD") b++} END{print n" dinilai, "b" BAD"}' $R/wallet_rep.tsv)"
echo "== koin seumur hidup: $(wc -l < $R/coins_alltime.tsv), rug-like (dev jual <=60 dtk): $(awk -F'\t' '$3>=0 && $3<=60' $R/coins_alltime.tsv | wc -l)"
echo "== asal dana: $(cut -f4 $R/origin.tsv | sort | uniq -c | tr '\n' ' ')"
echo "== link baru dari Bitquery: $(wc -l < $R/links_bitquery.tsv)"
echo "== watchlist sebelum: $(jq '.teams|group_by(.class)|map(.[0].class+"="+(length|tostring))|join(" ")' reports/web/watch_all.json)"
echo "== watchlist sesudah: $(jq '.teams|group_by(.class)|map(.[0].class+"="+(length|tostring))|join(" ")' $R/watch_clean.json)"
echo "== wallet dipantau sesudah: $(jq '[.teams[]|(.devs|length)+(.watch|length)+1]|add' $R/watch_clean.json)"
echo REP_DONE
