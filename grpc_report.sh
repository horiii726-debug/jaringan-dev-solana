#!/usr/bin/env bash
# writes data/shred/GRPC_RESULTS.md from the current gRPC data (creates, hits, rise); method in GRPC_SHREDSTREAM.md
cd /root/sol
D=data/shred; C=$D/creates.jsonl; R=$D/rise/rise.jsonl
S=$(tail -1 $D/rx.log)
utc() { jq -r '((.t_ms/1000) | strftime("%Y-%m-%d %H:%M:%S"))'; }
first=$(head -1 $C | utc); last=$(tail -1 $C | utc)
ncre=$(wc -l < $C); ncr=$(jq -r .creator $C | sort -u | wc -l)
nhit=$(cat $D/hits.jsonl 2>/dev/null | wc -l); ndyn=$(cat $D/dynamic_watch.jsonl 2>/dev/null | wc -l)
spam=$(jq -r '[.creator, .name] | @tsv' $C | sort | uniq -c | awk '$1>=2' | wc -l)
U=/tmp/rise_u.jsonl; jq -s -c "map(select(.x_entry)) | unique_by(.mint) | .[]" $R > $U 2>/dev/null; nr=$(wc -l < $U)
rm -f $D/GRPC_HASIL.md
{
echo "# ShredStream gRPC results — $(date -u '+%Y-%m-%d %H:%M') UTC"
echo
echo "Method and column meanings: [../../GRPC_SHREDSTREAM.md](../../GRPC_SHREDSTREAM.md). Paper mode (no buying)."
echo
echo "## Reception"
echo "- Data window: $first to $last UTC"
echo "- Messages received: $(echo $S | jq .msgs), $(echo $S | jq .mb) MB, last slot $(echo $S | jq .slot)"
echo "- Messages with the new transaction format (read via the fallback): $(echo $S | jq .decode_fail)"
echo "- **New coins caught: $ncre** from **$ncr distinct creators**"
echo "- Creators that launched the same name >= 2 times (spam-bot sign): $spam"
echo "- Wallets watched (watchlist + new wallets funded by a parent): $(echo $S | jq .watch)"
echo "- **Watchlist matches: $nhit** | new wallets funded by a parent: $ndyn"
echo
echo "## Coin rises (measured: $nr coins)"
if [ "$nr" -gt 0 ]; then
  jq -r .x_entry $U | sort -g | awk '{a[NR]=$1} END{n=NR;
    printf "- From the first reachable price (slot after the create) to the peak: min %+.0f%%, median %+.0f%%, 90%% of coins below %+.0f%%, max %+.0f%%\n", (a[1]-1)*100, (a[int(n/2)+1]-1)*100, (a[int(n*0.9)]-1)*100, (a[n]-1)*100;
    for(i=1;i<=n;i++){ if(a[i]<1.1)b1++; else if(a[i]<2)b2++; else if(a[i]<11)b3++; else b4++ }
    printf "- Rose < 10%%: %d | 10-100%%: %d | 100-1000%%: %d | >= 1000%%: %d\n", b1, b2, b3, b4}'
  echo "- Graduated to the DEX: $(jq -c "select(.graduated==true)" $U | wc -l)"
  echo "- Collapsed >= 90% from the peak and not graduated (rug-like): $(jq -c "select(.graduated!=true and (.last_vs_entry / .x_entry) <= 0.1)" $U | wc -l)"
  echo "- Last price below the entry price: $(jq -c "select(.last_vs_entry < 1)" $U | wc -l)"
  echo "- Stream creates that were an earlier failed attempt (the coin was created > 2 s later): $(jq -c "select(.delay_ms != null and .delay_ms < -2000)" $R | wc -l)"
  jq -r "select(.delay_ms != null and .delay_ms >= -2000) | .delay_ms" $U | sort -n | awk '{a[NR]=$1} END{ if (NR) printf "- Our delay vs block time: median %d ms, fastest 10%% <= %d ms, 90%% <= %d ms (block time is rounded to the second)\n", a[int(NR/2)+1], a[int(NR/10)+1], a[int(NR*0.9)]}'
  echo
  echo "### Top 20 by rise"
  echo "| Symbol | Rise from entry | Peak mcap (\$, SOL \$67.5) | Trades | Graduated | Mint |"
  echo "|---|---|---|---|---|---|"
  jq -r 'select(.x_entry) | [(.x_entry*100-100|floor), (.symbol|gsub("\\|";"/")), ((.peak_mcap_sol*67.5)|floor), .n_trades, (if .graduated then "yes" else "-" end), .mint] | @tsv' $U \
    | sort -t$'\t' -k1,1nr | head -20 | awk -F'\t' '{printf "| %s | +%s%% | %s | %s | %s | `%s` |\n", $2, $1, $3, $4, $5, $6}'
fi
echo
echo "## Latest 20 coins"
echo "| Time (UTC) | Symbol | Name | Creator | Watchlist match |"
echo "|---|---|---|---|---|"
tail -20 $C | jq -r '[((.t_ms/1000) | strftime("%H:%M:%S")), (.symbol|gsub("\\|";"/")), (.name|gsub("\\|";"/")|.[0:30]), .creator, (if .hit then .hit.entitas else "-" end)] | @tsv' \
  | awk -F'\t' '{printf "| %s | %s | %s | `%s` | %s |\n", $1, $2, $3, $4, $5}'
} > $D/GRPC_RESULTS.md
echo "written: $D/GRPC_RESULTS.md"
