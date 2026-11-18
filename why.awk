# Reverse engineering: what separates NAIK/MOONSHOT from RUGPULL/SEPI/TURUN at decision time (create tx + history).
# Inputs (ARGIND): 1 cats.tsv (mint cat) 2 team devs (one wallet per line) 3 feat_dj1.tsv (header, mint last column)
# Prints, for team coins and for all coins: medians per category, and rising rate per bucket of each key feature.
BEGIN { FS = "\t" }
ARGIND == 1 { cat[$1] = $2; next }
ARGIND == 2 { td[$1] = 1; next }
FNR == 1 { for (i = 1; i <= NF; i++) col[$i] = i; next }
{
  m = $NF; if (!(m in cat)) next
  c = cat[m]; up = (c == "MOONSHOT" || c == "NAIK")
  # creator is not in the feature file; team membership comes from the dev list via the launch map (ARGIND 2 holds mints of team coins)
  g = (m in td) ? "TIM" : "SEMUA"
  for (k = 1; k <= 2; k++) {
    G = (k == 1) ? "SEMUA" : g; if (k == 2 && g != "TIM") continue
    n[G, c]++; N[G]++; U[G] += up
    split("own_buy_sol tx_buy_sol tx_buyers user_ne_creator name_len prev_launch_creator secs_since_prev launch_24h_creator prev_launch_user heat_5m same_sym_24h fund_lag_s funder_prior_funded hour", F, " ")
    for (f in F) { v = $(col[F[f]]); vals[G, c, F[f]] = vals[G, c, F[f]] " " v }
    # buckets
    b1 = $(col["own_buy_sol"]); bb = (b1 == 0) ? "0" : (b1 < 0.5 ? "<0.5" : (b1 < 2 ? "0.5-2" : (b1 < 5 ? "2-5" : ">=5")))
    add(G, "dev_beli_sol", bb, up)
    t = $(col["tx_buyers"]); add(G, "bundle_wallet", (t <= 1 ? "0-1" : (t <= 3 ? "2-3" : ">=4")), up)
    s = $(col["same_sym_24h"]); add(G, "nama_tiruan_24j", (s == 0 ? "baru" : (s < 5 ? "1-4" : ">=5")), up)
    p = $(col["prev_launch_creator"]); add(G, "koin_sebelumnya_dev", (p == 0 ? "0" : (p < 3 ? "1-2" : ">=3")), up)
    q = $(col["secs_since_prev"]); add(G, "jeda_dari_koin_sebelumnya", (q < 0 ? "pertama" : (q < 600 ? "<10mnt" : (q < 86400 ? "<1hari" : ">=1hari"))), up)
    l = $(col["fund_lag_s"]); add(G, "jeda_dana_ke_create", (l < 0 ? "tdk_tahu" : (l < 300 ? "<5mnt" : (l < 86400 ? "<1hari" : ">=1hari"))), up)
    h = $(col["hour"]); add(G, "jam_UTC", (h < 6 ? "00-05" : (h < 12 ? "06-11" : (h < 18 ? "12-17" : "18-23"))), up)
  }
}
function add(G, f, b, up) { bn[G, f, b]++; bu[G, f, b] += up; keys[G, f, b] = 1 }
function med(s,   a, k) { k = split(s, a, " "); asort(a); return k ? a[int((k + 1) / 2)] : "-" }
END {
  for (G in N) {
    printf "\n=== %s: %d koin, naik %.1f%%\n", G, N[G], 100 * U[G] / N[G]
    printf "median per jenis:\n%-22s", "ciri"; split("MOONSHOT NAIK TURUN RUGPULL SEPI", C, " ")
    for (i = 1; i <= 5; i++) printf "%10s", C[i]; printf "\n"
    split("own_buy_sol tx_buy_sol tx_buyers prev_launch_creator launch_24h_creator same_sym_24h fund_lag_s heat_5m", F, " ")
    for (j = 1; j <= 8; j++) { printf "%-22s", F[j]; for (i = 1; i <= 5; i++) printf "%10s", med(vals[G, C[i], F[j]]); printf "\n" }
    printf "peluang naik per kelompok:\n"
    PROCINFO["sorted_in"] = "@ind_str_asc"
    for (k in keys) { split(k, z, SUBSEP); if (z[1] != G) continue; if (bn[k] >= 20) printf "  %-26s %-9s n=%5d naik=%5.1f%%\n", z[2], z[3], bn[k], 100 * bu[k] / bn[k] }
  }
}
