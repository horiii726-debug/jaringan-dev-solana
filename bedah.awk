# Dissect big coins: creator, funder chain root (exchanges stop the walk), creator's coin count, early insiders.
# ARGIND: 1 exchange_list  2 fund rows (w funder deep)  3 creator coin counts (creator n)  4 early repeats (buyer n)
#         5 early buys (mint buyer)  6 big_creators (mint creator user)  7 big_bq.tsv (mint max last vol trades buyers sym)
BEGIN { FS = "\t"; OFS = "\t" }
ARGIND == 1 { ex[$1] = 1; next }
ARGIND == 2 { fu[$1] = $2; dp[$1] = $3; next }
ARGIND == 3 { cc[$1] = $2; next }
ARGIND == 4 { rep[$1] = $2; next }
ARGIND == 5 { if (rep[$2] >= 3) ins[$1]++; nb[$1]++; next }
ARGIND == 6 { cr[$1] = $2; us[$1] = $3; next }
{
  m = $1; c = cr[m]; r = c; lv = 0; why = "akar"
  if (c == "") { why = "creator_tak_diketahui" }
  else while (lv < 8) {
    if (!(r in fu)) { why = (lv == 0 ? "funder_belum_dicek" : "akar"); break }
    nx = fu[r]; if (nx == "-" || nx == "") { why = "akar"; break }
    if (nx in ex) { why = "dana_dari_exchange"; break }
    r = nx; lv++
    if (dp[r] == "true") { why = "akar_wallet_besar"; break }
  }
  root[m] = r; rcount[r]++; if ($2 >= 1000000) r1m[r]++; if ($2 >= 10000000) r10m[r]++
  rows[m] = $7 OFS $2 OFS $4 OFS $6 OFS c OFS (cc[c] + 0) OFS r OFS lv OFS why OFS (ins[m] + 0) "/" (nb[m] + 0)
  order[++n] = m
}
END {
  print "symbol", "puncak_mcap_usd", "volume_usd", "pembeli", "creator", "koin_creator(6-15nov)", "akar_dana", "tingkat", "berhenti_karena", "insider_awal/pembeli_awal", "koin_besar_dari_akar_yg_sama" > "/dev/stderr"
  for (i = 1; i <= n; i++) { m = order[i]; print rows[m], rcount[root[m]] }
}
