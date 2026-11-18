# Dev report for the devs of the alive DEX coins.
# Inputs: 1 svc labels (w label n_sigs), 2 grad list (mint), 3 dex status (mint status), 4 first funding (w funder kind lam),
#         5 history (w complete mint bt)   [one row per created coin; w complete "-" "-" when none found]
BEGIN { FS = "\t"; oct = 1790812800 }   # 2026-10-01 00:00 UTC
ARGIND==1 { lab[$1]=$2; ns[$1]=$3; next }
ARGIND==2 { grad[$1]=1; next }
ARGIND==3 { st[$1]=$2; next }
ARGIND==4 { fu[$1]=$2; fk[$1]=$3; fl[$1]=$4; next }
ARGIND==5 { w=$1; dev[w]=1; comp[w]=$2; if ($3=="-") next; if (seen[$3]++) next
  n[w]++
  if ($4 < oct) { old[w]++; next }        # created before 1 Oct: graduation unknown in our data
  if ($3 in st) { s=st[$3]; dex[w]++; if (s=="hidup") alive[w]++; else if (s=="rug") rug[w]++; else sepi[w]++ }
  else if ($3 in grad) { dead[w]++ }      # graduated, no longer on DexScreener
  else fail[w]++                          # never left the launchpad
  next }
END {
  for (w in dev) {
    f=fu[w]; t = (f=="") ? "tidak_ada_data" : (fk[w]!="transfer" ? "tidak_langsung" : (f in lab ? lab[f] : "belum_dilabel"))
    if (t=="OPERATOR" && ns[f]>=1000) t="OPERATOR_ramai"
    ft[t]++
    tot += n[w]; tf += fail[w]; td += dex[w]+dead[w]; ta += alive[w]; tr += rug[w]; to += old[w]
    if (!comp[w] || comp[w]=="false") inc++
    b = (n[w]<=1) ? "1" : (n[w]<=5 ? "2-5" : (n[w]<=20 ? "6-20" : ">20")); nb[b]++
    if (dex[w]+dead[w] >= 2) rep++
    printf "%s\t%d\t%d\t%d\t%d\t%d\t%d\t%s\t%s\n", w, n[w], fail[w], dex[w]+dead[w], alive[w], rug[w], old[w], t, comp[w] > "/dev/stderr"
  }
  printf "dev=%d (riwayat terpotong >500 tx: %d)\n", length(dev), inc
  printf "koin dibuat total=%d | gagal di launchpad=%d (%.1f%%) | sampai DEX=%d (%.1f%%) | masih hidup=%d | rug di DEX=%d | sebelum 1 Okt (tak dinilai)=%d\n", tot, tf, 100*tf/(tot-to+1e-9), td, 100*td/(tot-to+1e-9), ta, tr, to
  printf "dev dengan >=2 koin sampai DEX: %d\n", rep
  printf "jumlah koin per dev:"; for (b in nb) printf " %s=%d", b, nb[b]; print ""
  printf "dana pertama dari:"; for (t in ft) printf " %s=%d", t, ft[t]; print ""
}
