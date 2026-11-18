#!/bin/bash
cd /root/jaringan
until grep -q CHAIN_DONE logs/origin.log && grep -q "DONE\|ERROR" logs/bqlink.log && [ -s /tmp/anow_mc.tsv ]; do sleep 30; done
cat data/hist/bq_links_a.jsonl >> data/hist/bq_links.jsonl 2>/dev/null
./rep.sh > reports/reputasi/ringkasan.txt 2>&1
if grep -q REP_DONE reports/reputasi/ringkasan.txt && [ -s reports/reputasi/watch_clean.json ]; then
  cp reports/web/watch_all.json reports/reputasi/watch_before.json
  cp reports/reputasi/watch_clean.json reports/web/watch_all.json
  cp -r reports/reputasi dataset/
  set -a; . ./.env; set +a; git add -A; ok=1
  for v in $(grep -E "^[A-Z_]+=" .env | cut -d= -f2-); do git grep -q --cached -F -- "$v" && ok=0; done
  if [ $ok = 1 ]; then git commit -q -m "Reputation from full wallet history, origin trace, post-graduation pool check; watchlist cleaned"
    printf #!/bin/shnecho n > /tmp/askpass.sh; chmod 700 /tmp/askpass.sh
    GIT_ASKPASS=/tmp/askpass.sh GIT_TERMINAL_PROMPT=0 git push -q origin main; rm -f /tmp/askpass.sh; echo PUSHED
  else git reset -q; echo SECRET_ABORT; fi
fi
echo FINAL_DONE
