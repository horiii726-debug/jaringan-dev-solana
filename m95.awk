# Interim performance of surviving moonshots, all values = market cap in SOL (supply 1e9).
# Entry = first curve trade after the create slot (reachable); if the coin graduated in the create slot: first pool minute close.
# Inputs (ARGIND): 1 m95.tsv (mint curve bt slot pool gbt gslot), 2 m95_curve.tsv (mint slot bt vsol vtok),
#                  3 m95_min.tsv (mint ts high close), 4 m95_hour.tsv (mint ts high close), 5 moon.jsonl as tsv (mint sym mcap_usd)
BEGIN { FS = "\t" }
ARGIND==1 { cbt[$1]=$3; cs[$1]=$4; gbt[$1]=$6; gs[$1]=$7; next }
ARGIND==2 { m=$1; v=$4/$5*1e6; nc[m]++; cslot[m][nc[m]]=$2; cbtt[m][nc[m]]=$3; cv[m][nc[m]]=v; next }
ARGIND==3 { m=$1; nm[m]++; mts[m][nm[m]]=$2; mh[m][nm[m]]=$3*1e9; mc[m][nm[m]]=$4*1e9; next }
ARGIND==4 { m=$1; nh[m]++; hts[m][nh[m]]=$2; hh[m][nh[m]]=$3*1e9; hc[m][nh[m]]=$4*1e9; next }
ARGIND==5 { sym[$1]=$2; now_usd[$1]=$3; next }
function at(m, t,   v, i) { v=-1
  for (i=1;i<=nc[m];i++) if (cbtt[m][i]<=t && cslot[m][i]>cs[m]) v=cv[m][i]
  for (i=1;i<=nm[m];i++) if (mts[m][i]+60<=t) v=mc[m][i]
  return v }
END {
  printf "sym\tentry_s\tentry_mcapSOL\tATH_x\tmin_to_ATH\t+1m_x\t+5m_x\t+15m_x\t+60m_x\tmin>=2x\tmin>=5x\tmin>=10x\tnow_x\tnow_usd\n"
  for (m in cbt) {
    # sort helpers: curve rows arrive in slot order from the trades file; candles are newest-first -> sort ascending
    if (nm[m]) { n=asorti(mts[m], idx, "@val_num_asc"); delete t2; delete h2; delete c2; for(i=1;i<=n;i++){t2[i]=mts[m][idx[i]]; h2[i]=mh[m][idx[i]]; c2[i]=mc[m][idx[i]]}
                 delete mts[m]; delete mh[m]; delete mc[m]; for(i=1;i<=n;i++){mts[m][i]=t2[i]; mh[m][i]=h2[i]; mc[m][i]=c2[i]} }
    e=-1; et=0
    for (i=1;i<=nc[m];i++) if (cslot[m][i]>cs[m]) { e=cv[m][i]; et=cbtt[m][i]; break }
    if (e<0 && nm[m]) { e=mc[m][1]; et=mts[m][1] }
    if (e<=0) { noentry++; continue }
    ath=e; tath=et
    for (i=1;i<=nc[m];i++) if (cv[m][i]>ath) { ath=cv[m][i]; tath=cbtt[m][i] }
    for (i=1;i<=nm[m];i++) if (mh[m][i]>ath) { ath=mh[m][i]; tath=mts[m][i] }
    for (i=1;i<=nh[m];i++) if (hh[m][i]>ath) { ath=hh[m][i]; tath=hts[m][i] }
    # minutes held above k x entry, first 1000 minutes of pool candles (carry last close over empty minutes)
    a2=a5=a10=0
    if (nm[m]) { j=1; last=-1; for (t=mts[m][1]; t<mts[m][1]+60000; t+=60) { while (j<=nm[m] && mts[m][j]<=t) { last=mc[m][j]; j++ } if (last>=2*e) a2++; if (last>=5*e) a5++; if (last>=10*e) a10++ } }
    nowv = nh[m] ? hc[m][nh[m]] : -1
    if (nh[m]) { hmax=0; for(i=1;i<=nh[m];i++) if (hts[m][i]>hmax){hmax=hts[m][i]; nowv=hc[m][i]} }
    x1=at(m,cbt[m]+60)/e; x5=at(m,cbt[m]+300)/e; x15=at(m,cbt[m]+900)/e; x60=at(m,cbt[m]+3600)/e
    printf "%s\t%d\t%.0f\t%.1f\t%.0f\t%.2f\t%.2f\t%.2f\t%.2f\t%d\t%d\t%d\t%.1f\t%.0f\n", sym[m], et-cbt[m], e, ath/e, (tath-cbt[m])/60, x1, x5, x15, x60, a2, a5, a10, nowv/e, now_usd[m]
  }
  if (noentry) printf "no_entry_price\t%d\n", noentry > "/dev/stderr"
}
