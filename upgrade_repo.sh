#!/usr/bin/env bash
# 21 Nov (owner: "upgrade, jangan hapus"): refresh every output to now and rewrite the old exports with the current,
# exchange/bot-cleaned V3 data. Nothing is deleted; superseded files get a pointer to their replacement.
set -e
cd /root/sol
./target/release/jaringan famview data/v3/fam2 data/v3/dev data/v3/view > /tmp/fv.json 2>&1
./target/release/jaringan reput data/v3/universe data/v3/dev data/v3/rise data/v3/view/families_active.jsonl data/v3/ath data/v3/reput > /tmp/rp.json 2>&1
mkdir -p data/v3/export
{ printf "family_root\twallet\tperan\tsekali_pakai\tjumlah_dev\tjam_sejak_launch\n"
  jq -r '.root as $r | .n_devs as $nd | .last_launch_h as $h | .members[] | [$r, .w, .role, (if .one_time then "ya" else "tidak" end), $nd, $h] | @tsv' data/v3/view/families_active.jsonl
} > data/v3/export/family_anggota.tsv

# 1. family_export/ (same three files as 18 Nov, now from the cleaned V3 families)
A=data/v3/export/family_anggota.tsv; E=data/v3/reput/entitas.tsv; C=data/v3/reput/creator_coins.tsv
awk -F'\t' -v OFS=',' '
  FNR==1 {f++; next}
  f==1 {nw[$1]++; if ($3=="PERANTARA") np[$1]++; next}
  f==2 && $2=="FAMILY" {
    gsub(",", " ", $18)
    print $1, $3, $4, ($19!="-" && $19+0<=720 ? "YA" : "TIDAK"), nw[$1]+0, $6, np[$1]+0, $8, $9, $10, $11, $17, $18, $19 }
' "$A" "$E" | { echo "induk,status,alasan_eliminasi,aktif_30hari,jumlah_wallet,jumlah_dev,jumlah_perantara,koin,berhasil_ath_50k,gagal,rug,ath_max_usd,koin_terbaik,jam_sejak_launch"; cat; } > family_export/family_ringkas.csv
awk -F'\t' -v OFS=',' '
  FNR==1 {f++; next}
  f==1 && $3=="DEV" {root[$2]=$1; next}
  f==2 && ($1 in root) { gsub(",", " ", $3); print root[$1], $2, $3, $1, $4, $5, $6, $9, $10, $12 }
' "$A" "$C" | { echo "induk,mint,simbol,dibuat_oleh,tanggal,lulus,ath_usd,status,rug,dev_jual_pertama_detik"; cat; } > family_export/koin_family.csv
awk -F'\t' -v OFS=',' '
  FNR==1 {f++; next}
  f==1 {k[$1]++; next}
  f==2 {print $1, $2, $3, $4, k[$2]+0}
' "$C" "$A" | { echo "induk,wallet,peran,sekali_pakai,koin_dibuat"; cat; } > family_export/wallet_family.csv

# 2. quality tables recomputed on today's data
./target/release/jaringan qdev data/v3/dev data/v3/rise data/v3/quality > /tmp/qd.json 2>&1 || true
./target/release/jaringan qfam data/v3/dev data/v3/rise data/v3/fam2/families.jsonl data/v3/quality/fam2 > /tmp/qf.json 2>&1 || true

# 3. data_gz/: compressed backup of today's V3 data (same split layout as before)
tar -czf /tmp/data_v3.tgz --exclude='data/v3/ath/old_formula*' --exclude='data/v3/ath/bak_quote' --exclude='*.bak*' \
  --exclude='*/run*.log' data/v3
mkdir -p data_gz && rm -f data_gz/data.tgz.p_* data_gz/MD5SUMS
split -b 20M /tmp/data_v3.tgz data_gz/data.tgz.p_
(cd data_gz && md5sum data.tgz.p_* > MD5SUMS)
printf '# data_gz\nCompressed backup of the V3 data as of %s (no longer the 18 Nov data).\nRestore: cat data.tgz.p_* > data.tgz && tar -xzf data.tgz\n' "$(date -u '+%Y-%m-%d %H:%M UTC')" > data_gz/README.md
# 4. (pre-V3 files were removed on 21 Nov; nothing superseded is left to annotate)
# 4b. gRPC results report (English)
bash grpc_report.sh
# 5. STATUS_V3.md (English): numbers of this very run
R=/tmp/rp.json; V=/tmp/fv.json; W=data/v3/fam2/wallet_class.jsonl
n() { jq -r "$1" "$2"; }
cat > STATUS_V3.md <<EOF
# V3 status — $(date -u '+%Y-%m-%d %H:%M UTC')

Generated automatically from the data at this commit (\`jaringan reput\`, \`jaringan famview\`).
Data collection is STILL RUNNING; numbers grow with later commits.

## Creators (only creators of coins seen on DexScreener)
- Creators: **$(n .creators $R)** (dropped, no DexScreener coin: $(n .creators_non_dexscreener_dropped $R))
- History not traced yet: $(n .creators_not_traced $R)
- Coins recorded: $(n .coins $R); coins with a measured ATH: $(n .coins_with_ath $R); ATH >= \$50k (real): $(n .ath_ge50k $R); >= \$100k: $(n .ath_ge100k $R); >= \$1M: $(n .ath_ge1m $R)
- Empty pools (graduated, price ~0): $(n .pool_kosong $R); doubtful ATH data: $(n .ath_data_ragu $R)
- Creator status: $(n '.status | to_entries | map("\(.key) \(.value)") | join(", ")' $R)
- Elimination reasons: $(n '.eliminasi_alasan | to_entries | map("\(.key) \(.value)") | join(", ")' $R)
  (SERING_RUG = rugs often, KOIN_SEPI = thin coins, JUAL_DETIK_AWAL = sells in the first seconds, DEV_BOT = launch bot)

## Families (one operator = one entity)
- Families active <= 30 days: **$(n .families_active $V)**, wallets in them: $(n .wallets_in_active_families $V)
- Wallet roles: $(n '.roles | to_entries | map("\(.key) \(.value)") | join(", ")' $V) (INDUK = funding parent, PERANTARA = intermediary, PENERIMA = receives profits)
- Independent active devs: $(n .independent_devs_active $V); one-time dev wallets: $(n .one_time_dev_wallets_active $V)
- Wallet classes checked: $(jq -r .class $W | sort | uniq -c | awk '{printf "%s %s, ", $2, $1}')(INFRA_LIST = exchange, INFRA_BUSY = bot / super-busy wallet, NON_WALLET = program account; none of them links a family)

## Watchlist
- Entities: **$(n .watchlist_entitas $R)** (families $(n .watchlist_family $R), independents $(n .watchlist_independen $R))
- Wallets watched (DEV + INDUK + PERANTARA): **$(n .watchlist_wallet $R)**
- Coins in the watchlist archive: $(n .watchlist_koin $R), successful (ATH >= \$50k): $(n .watchlist_koin_berhasil_ge50k $R)
- Entity status: $(n '.entitas | to_entries | map("\(.key) \(.value)") | join(", ")' $R)

## Done
- Creators & families from DexScreener coins, screen (rugs / thin coins / early sells / launch bots), watchlist + project archive.
- ATH formula 3 (validated +-5% vs GeckoTerminal); non-SOL pools, thin pools and buy-only pools excluded.
- ShredStream gRPC receiver: Pump.fun creates seen ~0.15-0.9 s after the block, matched to the watchlist (see GRPC_SHREDSTREAM.md).
- Rows broken by the Helius quota cleaned; jobs stop on quota; request rate sized to the key.

## Not done yet
- Data incomplete (ATH and dev histories still running; see "history not traced yet").
- Paper mode (simulated buy when a watchlist entity creates a coin, hold to the DEX, P/L and latency).
- Historical backtest + ML (will a watchlist entity's next coin rise again), walk-forward, simple-rule baseline.
- Mint of new-format (0x81) stream transactions and parent -> new-wallet transfers in that format.

## Problems
- Helius key limited to ~15 calls/s (slow); remaining credits must be checked on the Helius dashboard.
- Low vast.ai balance: the server stops when it runs out.
- ShredStream trials end 21 Nov; longer tests need a paid plan. The vast server cannot receive UDP.
- The VPS has no GitHub login (pushes go through the laptop).
- No watchlist entity has created a coin during the stream tests yet (matching not proven on a real case).
EOF
git add -A
git diff --cached --stat | tail -1
