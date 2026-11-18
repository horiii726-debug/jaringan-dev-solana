#!/bin/bash
# DexScreener-first (owner 18 Nov): start from coins already trading on DEXes (all launchpads), not from Pump.fun junk.
#  1 discovery: GeckoTerminal top pools by 24h volume for launchpad DEX venues (10 pages x 20 per venue)
#  2 DexScreener details per token; keep: pair created >= 1 Oct 2026, mcap >= $100k, liquidity >= $20k and >= 3% of mcap,
#    24h volume >= $1k  (= rising, not rugged, not quiet, not a thin-float fake cap)
#  3 create tx (Helius, oldest tx of mint): launchpad, creator / fee payer / signers
#  4 dev profile + funder chain (strict links, exchange / service / bot labelled and not expanded)
cd /root/jaringan || exit 1
D=data/dexfirst; mkdir -p $D/gt
VENUES="pumpswap pump-fun meteora-damm-v2 meteora-dbc meteora raydium raydium-launchlab letsbonk-fun moonshot moonit boop-fun bags-fm heaven"
: > $D/pools.tsv
for v in $VENUES; do
  for p in 1 2 3 4 5 6 7 8 9 10; do
    f=$D/gt/$v.$p.json
    [ -s $f ] || { for t in 1 2 3 4; do curl -s -m 60 "https://api.geckoterminal.com/api/v2/networks/solana/dexes/$v/pools?page=$p&sort=h24_volume_usd_desc" > $f; jq -e '.data' $f >/dev/null 2>&1 && break; sleep 20; done; sleep 2.5; }
    n=$(jq '.data|length' $f 2>/dev/null); [ "${n:-0}" = 0 ] && break
    jq -r --arg v $v '.data[] | [$v, (.relationships.base_token.data.id|sub("^solana_";"")), .attributes.address, .attributes.pool_created_at] | @tsv' $f >> $D/pools.tsv
  done
done
cut -f2 $D/pools.tsv | sort -u | grep -v -E '^So11111111111111111111111111111111111111112$|^EPjFWdd5|^Es9vMFrz' > $D/tokens.txt
echo "venue_pools=$(wc -l < $D/pools.tsv) tokens=$(wc -l < $D/tokens.txt)"; cut -f1 $D/pools.tsv | sort | uniq -c | tr '\n' ' '; echo
# 2 DexScreener
: > $D/ds.jsonl; rm -rf $D/parts; mkdir -p $D/parts; split -l 30 $D/tokens.txt $D/parts/p_
for f in $D/parts/p_*; do
  L=$(paste -sd, $f)
  for t in 1 2 3; do curl -s -m 40 "https://api.dexscreener.com/tokens/v1/solana/$L" > $f.json && jq -e 'type=="array"' $f.json >/dev/null 2>&1 && break; sleep 3; done
  jq -c 'group_by(.baseToken.address)[] | max_by(.liquidity.usd // 0) | {mint: .baseToken.address, sym: .baseToken.symbol, dex: .dexId, labels: .labels, mcap: (.marketCap // .fdv), liq: .liquidity.usd, vol24: .volume.h24, pair_created: .pairCreatedAt, url: .url}' $f.json >> $D/ds.jsonl 2>/dev/null
done
rm -rf $D/parts
OCT=$(date -u -d 2026-10-01 +%s)000
jq -c --argjson oct $OCT 'select((.pair_created//0)>=$oct and (.mcap//0)>=100000 and (.liq//0)>=20000 and ((.liq//0)/(.mcap//1))>=0.03 and (.vol24//0)>=1000)' $D/ds.jsonl | sort -u > $D/win.jsonl
jq -r .mint $D/win.jsonl | sort -u > $D/win_mints.txt
echo "dexscreener=$(wc -l < $D/ds.jsonl) winners_since_1Oct=$(wc -l < $D/win_mints.txt)"
# 3 create tx roles + launchpad
./target/release/jaringan roles $D/win_mints.txt $D/roles.jsonl | tail -n 1
jq -r .venue $D/roles.jsonl | sort | uniq -c | tr '\n' ' '; echo
# 4 dev profile + strict funder chain
jq -r 'select(.creator)|[.creator,.bt,.mint]|@tsv' $D/roles.jsonl > $D/devprof_in.tsv
./target/release/jaringan devprof $D/devprof_in.tsv $D/devprof.jsonl | tail -n 1
cut -f1 $D/devprof_in.tsv | sort -u > $D/lvl1.txt
for L in 1 2 3 4 5 6; do
  ./target/release/jaringan fund $D/lvl$L.txt $D/fund_l$L.jsonl > /dev/null 2>&1
  jq -r 'select(.funder!=null and .kind=="transfer" and (.lam//0)>=50000000)|.funder' $D/fund_l$L.jsonl | sort -u > $D/f$L.txt
  ./target/release/jaringan svc $D/f$L.txt $D/svc.jsonl > /dev/null 2>&1
  jq -r 'select(.label=="OPERATOR")|.w' $D/svc.jsonl | sort -u > $D/operators.txt
  grep -xF -f $D/operators.txt $D/f$L.txt > $D/lvl$((L+1)).txt
  echo "chain level $L: strict_funders=$(wc -l < $D/f$L.txt) operators=$(wc -l < $D/lvl$((L+1)).txt)"
  [ -s $D/lvl$((L+1)).txt ] || break
done
for f in checkpoints/credits/*.txt; do cat $f; echo; done | awk '{s+=$1} END{print "credits_ledger_total=" s}'
echo DEXFIRST_DONE
