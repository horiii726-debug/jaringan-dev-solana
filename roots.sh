#!/bin/bash
# Owner 18 Nov: DexScreener winners (rising, not quiet / rugged) from 1 Oct - 18 Nov -> roots.
#  1 all graduated Pump.fun coins (SOL quote) 1 Oct - 18 Nov
#  2 DexScreener now: survivors = mcap >= $100k, liquidity >= $20k, liq/mcap >= 3% (no thin-float fakes), 24h volume >= $1k
#  3 creator of EVERY graduated coin (pool coin_creator) + exact roles of survivors (create tx: creator, signer, fee payer)
#  4 funder chains: survivors' creators up to 6 levels, every level labelled (src/svc.rs) and exchange / service / bot
#    wallets NOT expanded (path ends there); all graduated creators 2 levels (to find the same founder behind other coins)
cd /root/jaringan || exit 1
D=data/roots; mkdir -p $D
cat data/mig_oct.jsonl data/raw/migrations.jsonl data/mig_new.jsonl | jq -r 'select(.quote=="So11111111111111111111111111111111111111112")|[.mint,.pool,.bt]|@tsv' | sort -u -k1,1 > $D/grad.tsv
cut -f1 $D/grad.tsv > $D/grad_mints.txt
echo "graduated=$(wc -l < $D/grad.tsv)"
# 2 DexScreener (reuse rows already fetched)
cp data/moonall/ds.jsonl $D/ds.jsonl 2>/dev/null
jq -r .mint $D/ds.jsonl | sort -u > $D/dsdone.txt
grep -vxF -f $D/dsdone.txt $D/grad_mints.txt > $D/dstodo.txt
rm -rf $D/parts; mkdir -p $D/parts; split -l 30 $D/dstodo.txt $D/parts/p_
one() {
  f=$1; L=$(paste -sd, "$f")
  for t in 1 2 3; do curl -s -m 40 "https://api.dexscreener.com/tokens/v1/solana/$L" > "$f.json" && jq -e 'type=="array"' "$f.json" >/dev/null 2>&1 && break; sleep 3; done
  jq -c 'group_by(.baseToken.address)[] | max_by(.liquidity.usd // 0) | {mint: .baseToken.address, sym: .baseToken.symbol, dex: .dexId, mcap: (.marketCap // .fdv), liq: .liquidity.usd, vol24: .volume.h24, pair_created: .pairCreatedAt}' "$f.json" > "$f.out" 2>/dev/null
}
export -f one
ls $D/parts/p_* | xargs -P 6 -I{} bash -c 'one {}'
cat $D/parts/*.out >> $D/ds.jsonl; rm -rf $D/parts
jq -c 'select((.mcap//0)>=100000 and (.liq//0)>=20000 and ((.liq//0)/(.mcap//1))>=0.03 and (.vol24//0)>=1000)' $D/ds.jsonl | sort -u > $D/surv.jsonl
jq -r .mint $D/surv.jsonl | sort -u > $D/surv_mints.txt
echo "dexscreener_rows=$(wc -l < $D/ds.jsonl) survivors=$(wc -l < $D/surv_mints.txt)"
# 3 creators
cut -f1,2 $D/grad.tsv > $D/grad_pools.tsv
[ -s $D/pool_creators.jsonl ] || ./target/release/jaringan poolcreator $D/grad_pools.tsv $D/pool_creators.jsonl | tail -n 1
./target/release/jaringan roles $D/surv_mints.txt $D/roles.jsonl | tail -n 1
# 4a survivors: chain up to 6 levels, labelled
jq -r 'select(.creator)|.creator' $D/roles.jsonl | sort -u > $D/s_lvl1.txt
for L in 1 2 3 4 5 6; do
  ./target/release/jaringan fund $D/s_lvl$L.txt $D/s_fund_l$L.jsonl > /dev/null 2>&1
  jq -r 'select(.funder!=null)|.funder' $D/s_fund_l$L.jsonl | sort -u > $D/s_f$L.txt
  ./target/release/jaringan svc $D/s_f$L.txt $D/svc.jsonl > /dev/null 2>&1
  jq -r 'select(.label=="OPERATOR")|.w' $D/svc.jsonl | sort -u > $D/operators.txt
  grep -xF -f $D/operators.txt $D/s_f$L.txt > $D/s_lvl$((L+1)).txt
  echo "survivor chain level $L: funders=$(wc -l < $D/s_f$L.txt) operators_to_expand=$(wc -l < $D/s_lvl$((L+1)).txt)"
  [ -s $D/s_lvl$((L+1)).txt ] || break
done
# 4b every graduated creator: 2 levels
jq -r 'select(.coin_creator)|.coin_creator' $D/pool_creators.jsonl | sort -u > $D/a_lvl1.txt
echo "all graduated creators=$(wc -l < $D/a_lvl1.txt)"
./target/release/jaringan fund $D/a_lvl1.txt $D/a_fund_l1.jsonl > /dev/null 2>&1
jq -r 'select(.funder!=null)|.funder' $D/a_fund_l1.jsonl | sort | uniq -c | awk '$1>=2{print $2}' > $D/a_f1_multi.txt
./target/release/jaringan svc $D/a_f1_multi.txt $D/svc.jsonl > /dev/null 2>&1
jq -r 'select(.funder!=null)|.funder' $D/a_fund_l1.jsonl | sort -u | grep -vxF -f <(jq -r 'select(.label!="OPERATOR" and .label!="UNKNOWN")|.w' $D/svc.jsonl) > $D/a_lvl2.txt
./target/release/jaringan fund $D/a_lvl2.txt $D/a_fund_l2.jsonl > /dev/null 2>&1
for f in checkpoints/credits/*.txt; do cat $f; echo; done | awk '{s+=$1} END{print "credits_ledger_total=" s}'
echo ROOTS_DONE
