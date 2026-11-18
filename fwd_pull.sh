#!/bin/bash
# Fresh holdout (never seen): Pump.fun creates in Bitquery EAP window, their creators funders, and trades of
# watchlist-linked coins + a random baseline. Output data/fwd/*.jsonl. Token from .env (never printed).
cd /root/jaringan || exit 1
. ./.env
D=data/fwd; mkdir -p $D
q() { curl -s -m 300 https://streaming.bitquery.io/eap -H "content-type: application/json" -H "Authorization: Bearer $BITQUERY_TOKEN" --data-binary @-; }
PUMP=6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P
SINCE=${SINCE:-2026-11-17T17:00:00Z}; NOW=$(date -u +%Y-%m-%dT%H:%M:%SZ)
# 1) creates, hour by hour
: > $D/creates.jsonl
t=$(date -u -d "$SINCE" +%s); end=$(date -u -d "$NOW" +%s)
while [ $t -lt $end ]; do
  a=$(date -u -d @$t +%Y-%m-%dT%H:%M:%SZ); b=$(date -u -d @$((t+3600)) +%Y-%m-%dT%H:%M:%SZ)
  printf "{\"query\":\"{ Solana { Instructions(limit:{count:10000}, orderBy:{ascending:Block_Time}, where:{Block:{Time:{since:\\\\\"%s\\\\\", till:\\\\\"%s\\\\\"}}, Transaction:{Result:{Success:true}}, Instruction:{Program:{Address:{is:\\\\\"%s\\\\\"},Method:{in:[\\\\\"create\\\\\",\\\\\"create_v2\\\\\"]}}}}) { Block { Time Slot } Transaction { Signer Signature Index } Instruction { Accounts { Address } } } } }\"}" "$a" "$b" "$PUMP" | q > $D/raw_c.json
  jq -c ".data.Solana.Instructions[] | {mint: .Instruction.Accounts[0].Address, curve: .Instruction.Accounts[2].Address, user: .Instruction.Accounts[5].Address, signer: .Transaction.Signer, slot: (.Block.Slot|tonumber), bt: (.Block.Time|fromdateiso8601), txi: .Transaction.Index, sig: .Transaction.Signature}" $D/raw_c.json >> $D/creates.jsonl || { echo "creates error $a"; head -c 300 $D/raw_c.json; }
  echo "creates $a n=$(jq ".data.Solana.Instructions|length" $D/raw_c.json)"
  t=$((t+3600))
done
# 2) funders: earliest native SOL transfer received by each creator (within the window)
jq -r ".signer" $D/creates.jsonl | sort -u > $D/creators.txt
: > $D/funders.jsonl
split -l 800 $D/creators.txt $D/cr_
for f in $D/cr_*; do
  L=$(sed "s/.*/\\\\\"&\\\\\"/" $f | paste -sd,)
  printf "{\"query\":\"{ Solana { Transfers(limitBy:{by:Transfer_Receiver_Address, count:3}, orderBy:{ascending:Block_Time}, where:{Block:{Time:{since:\\\\\"2026-11-17T00:00:00Z\\\\\"}}, Transaction:{Result:{Success:true}}, Transfer:{Receiver:{Address:{in:[%s]}}, Currency:{Native:true}, Amount:{ge:\\\\\"0.01\\\\\"}}}) { Block { Time } Transfer { Amount Sender { Address } Receiver { Address } } } } }\"}" "$L" | q > $D/raw_f.json
  jq -c ".data.Solana.Transfers[] | {w: .Transfer.Receiver.Address, funder: .Transfer.Sender.Address, sol: (.Transfer.Amount|tonumber), bt: (.Block.Time|fromdateiso8601)}" $D/raw_f.json >> $D/funders.jsonl || { echo "funders error"; head -c 300 $D/raw_f.json; }
done
rm -f $D/cr_*
echo "creators=$(wc -l < $D/creators.txt) funder_rows=$(wc -l < $D/funders.jsonl)"
