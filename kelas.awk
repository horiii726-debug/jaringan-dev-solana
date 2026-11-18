# Class A / B / C for dev wallets and teams from their coin history (6-16 Nov). Rules (frozen v1, 17 Nov):
#  A: >= 2 coins held >= $50k (thousands of % from creation), 0 rugpulls, quiet coins <= 20% of coins with data
#  B: >= 1 coin held >= $50k, rugpulls <= 10%, quiet coins <= 40%
#  C: rising coins (NAIK or MOONSHOT or >= $50k) >= 30% of coins with data, rugpulls <= 25%
#  anything else is not listed (dropped)
# ARGIND: 1 cats (mint cat)  2 min50k (mint ...)  3 team members (team dev)  4 launches (mint creator)
BEGIN { FS = "\t"; OFS = "\t" }
ARGIND == 1 { cat[$1] = $2; next }
ARGIND == 2 { big[$1] = 1; next }
ARGIND == 3 { tm[$2] = $1; next }
{
  m = $1; c = $2
  for (pass = 1; pass <= 2; pass++) {
    k = (pass == 1) ? "DEV\t" c : ((c in tm) ? "TIM\t" tm[c] : ""); if (k == "") continue
    n[k]++
    if (m in big) b[k]++
    if (m in cat) { d[k]++; x = cat[m]; if (x == "RUGPULL") r[k]++; if (x == "SEPI") s[k]++; if (x == "NAIK" || x == "MOONSHOT" || (m in big)) u[k]++ }
    else if (m in big) { d[k]++; u[k]++ }
  }
}
END {
  for (k in n) {
    dd = d[k] + 0; if (dd == 0) continue
    rr = r[k] + 0; ss = s[k] + 0; bb = b[k] + 0; uu = u[k] + 0
    cls = ""
    if (bb >= 2 && rr == 0 && ss <= 0.2 * dd) cls = "A"
    else if (bb >= 1 && rr <= 0.1 * dd && ss <= 0.4 * dd) cls = "B"
    else if (uu >= 0.3 * dd && rr <= 0.25 * dd && dd >= 2) cls = "C"
    if (cls != "") print cls, k, n[k], dd, bb, uu, rr, ss
  }
}
