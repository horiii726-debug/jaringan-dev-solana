# input (sorted by score asc): label<TAB>score ; Mann-Whitney AUC with average ranks for ties
{ lab[NR]=$1; sc[NR]=$2 }
END { n=NR; i=1; while (i<=n) { j=i; while (j<n && sc[j+1]==sc[i]) j++; r=(i+j)/2; for(k=i;k<=j;k++) if(lab[k]==1){p++; rs+=r}; i=j+1 }
  q=n-p; if(p&&q) printf "%.3f", (rs-p*(p+1)/2)/(p*q); else printf "-" }
