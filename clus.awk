# Syndicate vs independent devs (devs whose coin reached the DEX and is alive).
# Edges dev -> X only through wallets labelled OPERATOR (exchanges, services, bot hubs never link):
#   F = first funding (direct transfer >= 0.05 SOL), I = funding received before launch (>= 0.05 SOL),
#   O = money sent after launch (>= 0.1 SOL).
# Syndicate = devs sharing a linking wallet; STRONG if the devs share >= 2 different signal types.
# Inputs: 1 svc labels (w<TAB>label), 2 edges (dev<TAB>X<TAB>type)
BEGIN { FS = "\t" }
ARGIND==1 { lab[$1]=$2; next }
ARGIND==2 { d=$1; x=$2; t=$3; dev[d]=1; if (lab[x]!="OPERATOR" || x==d) next; key=d SUBSEP x; if (!(key in seen)) { seen[key]=1; nx[x]++; xs[x]=xs[x] " " d } types[x]=types[x] t; next }
function find(a) { while (p[a]!="" && p[a]!=a) a=p[a]; return a }
function uni(a,b) { a=find(a); b=find(b); if (a!=b) p[a]=b }
END {
  for (d in dev) p[d]=d
  for (x in nx) if (nx[x]>=2) { n=split(xs[x], ds, " "); for (i=2;i<=n;i++) uni(ds[1], ds[i]); for (i=1;i<=n;i++) { r=ds[i]; sig[r]=sig[r] types[x] } }
  for (d in dev) { r=find(d); size[r]++; s[r]=s[r] sig[d] }
  for (r in size) {
    if (size[r]==1) { indep++; continue }
    syn++; members+=size[r]
    t=0; if (s[r] ~ /F/) t++; if (s[r] ~ /I/) t++; if (s[r] ~ /O/) t++
    if (t>=2) strong++
    if (size[r]>=5) big++
    printf "SYNDIKAT\tdev=%d\tsinyal=%s\n", size[r], (s[r] ~ /F/ ? "F" : "") (s[r] ~ /I/ ? "I" : "") (s[r] ~ /O/ ? "O" : "") > "/dev/stderr"
  }
  printf "dev_dianalisis=%d\nsindikat=%d (kuat_>=2_sinyal=%d, besar_>=5_dev=%d) berisi %d dev\nwallet_independen=%d\n", length(dev), syn, strong, big, members, indep
}
