#!/bin/bash
cd /root/jaringan || exit 1
D=data/roots
until grep -q DEVHIST_DONE $D/devhist.log; do sleep 30; done
jq -r "[.w,.label,.n_sigs]|@tsv" $D/svc.jsonl > $D/lab3.tsv
cut -f1 $D/grad.tsv > $D/grad_m.txt
jq -r "[.mint, (if (.liq//0)<2000 then \"rug\" elif ((.mcap//0)>=100000 and (.liq/.mcap)<0.03) then \"palsu\" elif (.vol24//0)<1000 then \"sepi\" else \"hidup\" end)]|@tsv" $D/ds.jsonl > $D/dexst.tsv
jq -r "[.w, (.funder//\"\"), (.kind//\"\"), (.lam//0)]|@tsv" $D/alive_fund.jsonl > $D/ff.tsv
jq -r ".w as \$w | .complete as \$c | if (.creates|length)==0 then [\$w, (\$c|tostring), \"-\", \"-\"] else (.creates[] | [\$w, (\$c|tostring), .mint, (.bt|tostring)]) end | @tsv" $D/alive_hist.jsonl > $D/hist.tsv
gawk -f devrep.awk $D/lab3.tsv $D/grad_m.txt $D/dexst.tsv $D/ff.tsv $D/hist.tsv 2> $D/dev_table.tsv
echo "-- dev teratas (koin sampai DEX terbanyak): wallet total gagal dex hidup rug sebelumOkt danaPertama riwayatLengkap"
sort -t"	" -k4,4nr $D/dev_table.tsv | head -15 | awk -F"\t" "{printf \"%s %s %s %s %s %s %s %s %s\n\", substr(\$1,1,8),\$2,\$3,\$4,\$5,\$6,\$7,\$8,\$9}"
echo DEVREP_DONE
