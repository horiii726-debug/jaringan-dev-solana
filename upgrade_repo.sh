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
printf "# data_gz\nCadangan terkompresi data V3 per %s (bukan lagi data 18 Nov).\nGabung: cat data.tgz.p_* > data.tgz && tar -xzf data.tgz\n" "$(date -u '+%Y-%m-%d %H:%M UTC')" > data_gz/README.md

# 4. superseded files keep their content, with a pointer on top
note() { f=$1; shift; grep -q "^> DIGANTIKAN" "$f" 2>/dev/null || { printf "> DIGANTIKAN (21 Nov): %s\n\n" "$*" | cat - "$f" > /tmp/n && mv /tmp/n "$f"; }; }
for f in STATUS.md HASIL_DAN_MASALAH.md HASIL_ML_V3.md LANJUT_ML_AUDIT.md data_ringkas_18nov.md; do
  [ -f "$f" ] && note "$f" "versi 18 Nov; angka & status terbaru di STATUS_V3.md dan README.md"
done
[ -f modul_18nov/README.md ] && note modul_18nov/README.md "salinan modul 18 Nov; versi aktif ada di src/"
for d in data/v3/unified data/v3/family; do [ -d $d ] || continue
  printf "# %s\nHasil antara versi sebelumnya (20 Nov). Digantikan oleh data/v3/fam2 (family, bersih exchange/bot),\ndata/v3/view (family aktif) dan data/v3/reput (reputasi + watchlist).\n" "$d" > $d/README.md
done
git add -A
git diff --cached --stat | tail -1
