# Teams (bubblemap groups) behind coins that held >= $50k: dev/creator -> funder chain -> root (exchange stops it).
# ARGIND: 1 exchange_list  2 fund rows (w funder deep)  3 min50k_fix (mint ...)  4 peak_mcap (mint sol usd)
#         5 cats (mint cat)  6 launches (mint creator)
# Output: one row per team with >= 1 member coin >= $50k; history over ALL coins of member devs (6-16 Nov).
BEGIN { FS = "\t"; OFS = "\t" }
ARGIND == 1 { ex[$1] = 1; next }
ARGIND == 2 { fu[$1] = $2; dp[$1] = $3; next }
ARGIND == 3 { big[$1] = 1; next }
ARGIND == 4 { usd[$1] = $3; next }
ARGIND == 5 { cat[$1] = $2; next }
{
  m = $1; c = $2
  if (!(c in rootof)) {
    r = c; lv = 0; kind = "MANDIRI"
    while (lv < 8) {
      if (!(r in fu)) { kind = (lv == 0 ? "BELUM_DICEK" : "TIM"); break }
      nx = fu[r]; if (nx == "-" || nx == "") { kind = (lv == 0 ? "MANDIRI" : "TIM"); break }
      if (nx in ex) { kind = (lv == 0 ? "DANA_EXCHANGE" : "TIM"); break }
      r = nx; lv++; kind = "TIM"
      if (dp[r] == "true") break
    }
    rootof[c] = r; kindof[c] = kind
  }
  R = rootof[c]; key = (kindof[c] == "TIM") ? R : c
  K[key] = (kindof[c] == "TIM") ? "TIM" : kindof[c]
  if (!((key, c) in mem)) { mem[key, c] = 1; nd[key]++ }
  n[key]++
  if (m in big) { b[key]++; if (usd[m] >= 1000000) m1[key]++; if (usd[m] >= 10000000) m10[key]++; if (usd[m] > best[key]) { best[key] = usd[m] } }
  if (m in cat) { known[key]++; if (cat[m] == "RUGPULL") rug[key]++; if (cat[m] == "SEPI") sepi[key]++; if (cat[m] == "TURUN") turun[key]++ }
}
END {
  print "jenis", "akar_atau_dev", "dev", "koin_total", "koin_min50k", "koin_min1M", "koin_min10M", "terbaik_usd", "riwayat_ada_data", "ragpul", "sepi", "turun", "persen_min50k" > "/dev/stderr"
  for (km in mem) { split(km, z, SUBSEP); if (b[z[1]] >= 2) print z[1], z[2] > "reports/team_members.tsv" }
  for (k in b) {
    pct = 100 * b[k] / n[k]
    print K[k], k, nd[k], n[k], b[k], m1[k] + 0, m10[k] + 0, best[k] + 0, known[k] + 0, rug[k] + 0, sepi[k] + 0, turun[k] + 0, sprintf("%.1f", pct)
  }
}
