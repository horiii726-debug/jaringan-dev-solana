#!/bin/bash
# creators -> earliest native SOL funders (Bitquery, batches of 400, retry on rate limit)
cd /root/jaringan || exit 1
. ./.env
D=data/fwd
q() { curl -s -m 300 https://streaming.bitquery.io/eap -H "content-type: application/json" -H "Authorization: Bearer $BITQUERY_TOKEN" --data-binary @-; }
: > $D/funders.jsonl
split -l 400 $D/creators.txt $D/cr_
for f in $D/cr_*; do
  L=$(sed "s/.*/\\\\\"&\\\\\"/" $f | paste -sd,)
  for try in 1 2 3 4 5 6; do
    printf "{\"query\":\"{ Solana { Transfers(limitBy:{by:Transfer_Receiver_Address, count:3}, orderBy:{ascending:Block_Time}, where:{Transaction:{Result:{Success:true}}, Transfer:{Receiver:{Address:{in:[%s]}}, Currency:{Native:true}, Amount:{ge:\\\\\"0.01\\\\\"}}}) { Block { Time } Transfer { Amount Sender { Address } Receiver { Address } } } } }\"}" "$L" | q > $D/raw_f.json
    if jq -e ".data.Solana.Transfers" $D/raw_f.json > /dev/null 2>&1; then
      jq -c ".data.Solana.Transfers[] | {w: .Transfer.Receiver.Address, funder: .Transfer.Sender.Address, sol: (.Transfer.Amount|tonumber), bt: (.Block.Time|fromdateiso8601)}" $D/raw_f.json >> $D/funders.jsonl; break
    fi
    echo "retry $f $try: $(head -c 120 $D/raw_f.json)"; sleep 30
  done
  sleep 8
done
rm -f $D/cr_*
echo "funder_rows=$(wc -l < $D/funders.jsonl) wallets_with_funder=$(jq -r .w $D/funders.jsonl | sort -u | wc -l)"
