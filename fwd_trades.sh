#!/bin/bash
# trades of target mints (Bitquery DEXTradeByTokens, SOL side only, up to CAP per mint, batches of 20, retry)
cd /root/jaringan || exit 1
. ./.env
D=data/fwd; CAP=${CAP:-1500}
q() { curl -s -m 300 https://streaming.bitquery.io/eap -H "content-type: application/json" -H "Authorization: Bearer $BITQUERY_TOKEN" --data-binary @-; }
touch $D/trades.jsonl
jq -r .mint $D/trades.jsonl 2>/dev/null | sort -u > $D/done.txt
grep -vxF -f $D/done.txt $D/targets.txt | grep . > $D/todo.txt
echo "todo=$(wc -l < $D/todo.txt)"
split -l 20 $D/todo.txt $D/tm_
for f in $D/tm_*; do
  L=$(sed "s/.*/\\\\\"&\\\\\"/" $f | paste -sd,)
  for try in 1 2 3 4 5 6; do
    printf "{\"query\":\"{ Solana { DEXTradeByTokens(limitBy:{by:Trade_Currency_MintAddress, count:%d}, orderBy:{ascending:Block_Time}, where:{Transaction:{Result:{Success:true}}, Trade:{Currency:{MintAddress:{in:[%s]}}, Side:{Currency:{MintAddress:{in:[\\\\\"11111111111111111111111111111111\\\\\",\\\\\"So11111111111111111111111111111111111111112\\\\\"]}}}}}) { Block { Time Slot } Trade { Price Currency { MintAddress } Side { Type AmountInUSD Amount } Dex { ProtocolName } } } } }\"}" $CAP "$L" | q > $D/raw_t.json
    if jq -e ".data.Solana.DEXTradeByTokens" $D/raw_t.json > /dev/null 2>&1; then
      jq -c ".data.Solana.DEXTradeByTokens[] | {mint: .Trade.Currency.MintAddress, slot: (.Block.Slot|tonumber), bt: (.Block.Time|fromdateiso8601), price: .Trade.Price, buy: (.Trade.Side.Type==\"buy\"), sol: (.Trade.Side.Amount|tonumber), amm: (.Trade.Dex.ProtocolName != \"pump\"), dex: .Trade.Dex.ProtocolName}" $D/raw_t.json > $D/batch.jsonl
      cat $D/batch.jsonl >> $D/trades.jsonl
      jq -r .mint $D/batch.jsonl | sort | uniq -c | awk -v c=$CAP "\$1>=c{print \"{\\\"mint\\\":\\\"\"\$2\"\\\",\\\"capped\\\":true}\"}" >> $D/trades.jsonl
      for m in $(cat $f); do grep -q "\"$m\"" $D/batch.jsonl || echo "{\"mint\":\"$m\",\"none\":true}" >> $D/trades.jsonl; done
      break
    fi
    echo "retry $try: $(head -c 120 $D/raw_t.json)"; sleep 30
  done
  sleep 6
done
rm -f $D/tm_*
echo "trade_rows=$(wc -l < $D/trades.jsonl)"
