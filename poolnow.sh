#!/bin/bash
# Current pool reserves (SOL and token) of each class-A coin, on-chain via Helius (single calls).
cd /root/jaringan
set -a; . ./.env; set +a
U="https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY"
: > /tmp/anow.tsv
q() { for i in 1 2 3 4 5 6; do r=$(curl -s -m 20 -X POST "$U" -H "content-type: application/json" -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTokenAccountsByOwner\",\"params\":[\"$1\",{\"mint\":\"$2\"},{\"encoding\":\"jsonParsed\"}]}"); if echo "$r" | jq -e '.result' >/dev/null 2>&1; then echo "$r" | jq -r '.result.value[0].account.data.parsed.info.tokenAmount.uiAmount // 0'; return; fi; sleep 2; done; echo -1; }
while IFS=$'\t' read -r m p; do
  s=$(q "$p" So11111111111111111111111111111111111111112); t=$(q "$p" "$m")
  printf "%s\t%s\t%s\n" "$m" "$s" "$t" >> /tmp/anow.tsv
done < /tmp/apools.tsv
awk -F'\t' '$2>=0 && $3>=0{mc=($3>0? $2/$3*1e9*71.08 : 0); print $1"\t"$2"\t"int(mc)}' /tmp/anow.tsv > /tmp/anow_mc.tsv
cp /tmp/anow_mc.tsv reports/reputasi_pool_now.tsv
awk -F'\t' '{n++; if($2<5) dr++; if($3<10000) lo++; if($3>=50000) hi++} END{print "koin_A_dicek="n" pool_dikuras(<5 SOL)="dr+0" mcap_sekarang<10k="lo+0" mcap_sekarang>=50k="hi+0}' /tmp/anow_mc.tsv
echo POOL_DONE
