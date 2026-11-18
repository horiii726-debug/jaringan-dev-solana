# Entity scoring (exploratory): per funder and per bundle wallet, outcomes of their coins.
# Inputs (ARGIND): 1 wf.tsv (w funder kind) 2 lm.tsv (mint creator user buyers;) 3 peak.tsv (mint peakmult grad) 4 j1.tsv (mint ret)
# GOOD = Wilson 95% lower bound of the >=2x rate above the base rate, n >= 5.
function wl(k, n,   p, z, d, c, h) { if (n == 0) return 0; z = 1.96; p = k / n; d = 1 + z*z/n; c = (p + z*z/(2*n)) / d; h = z*sqrt((p*(1-p) + z*z/(4*n))/n) / d; return c - h }
BEGIN { FS = "\t" }
ARGIND == 1 { fu[$1] = $2; next }
ARGIND == 2 { cr[$1] = $2; us[$1] = $3; by[$1] = $4; next }
ARGIND == 3 { pk[$1] = $2; next }
ARGIND == 4 { r[$1] = $2; next }
END {
  for (m in pk) {
    if (!(m in cr)) continue
    N++; S += pk[m]; B2 += (pk[m] >= 2); B10 += (pk[m] >= 10); RJ += r[m]
    c = cr[m]
    if (c in fu) { f = fu[c]; add("F", f, m) }
    nb = split(by[m], b, ";")
    delete seen
    for (i = 1; i <= nb; i++) if (b[i] != "" && b[i] != c && b[i] != us[m] && !(b[i] in seen)) { seen[b[i]] = 1; add("B", b[i], m) }
  }
  base2 = B2 / N
  printf "BASE koin=%d rata_puncak=%.2fx naik2x=%.1f%% naik10x=%.2f%% hasil_j1=%+.1f%%\n", N, S/N, 100*base2, 100*B10/N, 100*RJ/N
  for (t = 1; t <= 2; t++) {
    T = (t == 1) ? "F" : "B"; ne = 0; ng = 0; gc = 0; gs = 0; g2 = 0; g10 = 0; gr = 0
    for (k in n) { split(k, kk, SUBSEP); if (kk[1] != T || n[k] < 5) continue
      ne++
      if (wl(c2[k], n[k]) > base2) { ng++; gc += n[k]; gs += s[k]; g2 += c2[k]; g10 += c10[k]; gr += rj[k]
        printf "  %s GOOD %s koin=%d rata_puncak=%.2fx naik2x=%d naik10x=%d hasil_j1=%+.1f%%\n", T, substr(kk[2],1,8), n[k], s[k]/n[k], c2[k], c10[k], 100*rj[k]/n[k] }
    }
    printf "%s: entitas_dgn>=5_koin=%d GOOD=%d koin_GOOD=%d", (T=="F"?"FUNDER":"BUNDLE_WALLET"), ne, ng, gc
    if (gc) printf " rata_puncak=%.2fx naik2x=%.1f%% naik10x=%d hasil_j1=%+.1f%%", gs/gc, 100*g2/gc, g10, 100*gr/gc
    printf "\n"
  }
}
function add(T, e, m) { k = T SUBSEP e; n[k]++; s[k] += pk[m]; c2[k] += (pk[m] >= 2); c10[k] += (pk[m] >= 10); rj[k] += r[m] }
