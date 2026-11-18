#!/bin/bash
# moonshot coins (>= $50k) -> early trades (Bitquery) + funder chain 3 levels (Shyft RPC, full history), exchanges excluded
cd /root/jaringan || exit 1
. ./.env
D=data/fwd/moon
q() { curl -s -m 300 https://streaming.bitquery.io/eap -H "content-type: application/json" -H "Authorization: Bearer $BITQUERY_TOKEN" --data-binary @-; }
: > $D/early.jsonl
split -l 20 $D/mints.txt $D/mm_
for f in $D/mm_*; do
  L=$(sed "s/.*/\\\\\"&\\\\\"/" $f | paste -sd,)
  for try in 1 2 3 4 5; do
    printf "{\"query\":\"{ Solana { DEXTradeByTokens(limitBy:{by:Trade_Currency_MintAddress, count:60}, orderBy:{ascending:Block_Time}, where:{Transaction:{Result:{Success:true}}, Trade:{Currency:{MintAddress:{in:[%s]}}, Side:{Currency:{MintAddress:{in:[\\\\\"11111111111111111111111111111111\\\\\",\\\\\"So11111111111111111111111111111111111111112\\\\\"]}}}}}) { Block { Time Slot } Transaction { Signer Index } Trade { Price PriceInUSD Currency { MintAddress } Side { Type Amount } Account { Owner } Dex { ProtocolName } } } } }\"}" "$L" | q > $D/raw_e.json
    if jq -e ".data.Solana.DEXTradeByTokens" $D/raw_e.json >/dev/null 2>&1; then
      jq -c ".data.Solana.DEXTradeByTokens[] | {mint: .Trade.Currency.MintAddress, slot: (.Block.Slot|tonumber), bt: (.Block.Time|fromdateiso8601), txi: .Transaction.Index, price: .Trade.Price, usd: .Trade.PriceInUSD, buy: (.Trade.Side.Type==\"buy\"), sol: (.Trade.Side.Amount|tonumber), owner: .Trade.Account.Owner, signer: .Transaction.Signer, dex: .Trade.Dex.ProtocolName}" $D/raw_e.json >> $D/early.jsonl; break
    fi
    sleep 30
  done
  sleep 6
done
rm -f $D/mm_*
export JAR_RPC_URL="https://rpc.shyft.to?api_key=$SHYFT_KEY" JAR_RPS=3
cp $D/creators.txt $D/lvl1.txt
for L in 1 2 3; do
  ./target/release/jaringan fund $D/lvl$L.txt $D/fund_l$L.jsonl 2>&1 | sed "s/$SHYFT_KEY/***/g" | tail -1
  jq -r "select(.funder!=null)|.funder" $D/fund_l$L.jsonl | sort -u | grep -vxF -f dataset/wallets/exchange_list.txt > $D/lvl$((L+1)).txt
  echo "level $L funders=$(wc -l < $D/lvl$((L+1)).txt)"
done
echo TRACE_DONE
