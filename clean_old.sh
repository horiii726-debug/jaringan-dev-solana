#!/usr/bin/env bash
# 21 Nov (owner: "yang 3 hari lalu tidak diperbarui, hapus"): remove superseded pre-V3 files from the repo.
# Kept: code the binary needs (src/, idl/, Cargo.*), project rules (CLAUDE-CODE-*.md, .env.example), config/,
# shredrx/, checkpoints/, data/v3 (current), the upgraded family_export/ + data_gz/, README/STATUS_V3/RINGKASAN_V3,
# and the scripts that run V3. Git history keeps everything removed here.
set -e
cd /root/sol
keep_sh="start_all.sh start_ath.sh start_shredrx.sh stop_ath.sh stop_helius.sh reput_loop.sh probe24.sh clean_quote.sh restart_dev4.sh restart_f3.sh restart_fam2.sh restart_reput.sh restart_v7.sh upgrade_repo.sh clean_old.sh"
for f in $(git ls-files -- '*.sh' '*.awk' '*.py' | grep -v /); do
  case " $keep_sh " in *" $f "*) ;; *) git rm -q -f "$f" ;; esac
done
git rm -r -q -f --ignore-unmatch modul_18nov data/v3/unified data/v3/family watch_target.sh \
  STATUS.md HASIL_DAN_MASALAH.md HASIL_ML_V3.md LANJUT_ML_AUDIT.md data_ringkas_18nov.md
git add -A
echo "== dihapus: $(git diff --cached --name-only --diff-filter=D | wc -l) file"
git diff --cached --name-only --diff-filter=D | sed 's#/.*##' | sort | uniq -c | sort -rn | head -12
