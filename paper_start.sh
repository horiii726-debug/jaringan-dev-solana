#!/bin/bash
# Restart the monitor+paper executor with the full A/B/C watchlist, then back up to GitHub (secret-checked).
cd /root/jaringan
. /root/.cargo/env
while pgrep -f "[c]argo " > /dev/null; do sleep 5; done
cargo build --release > logs/paper_build.log 2>&1 || { echo BUILD_FAIL; exit 1; }
set -a; . ./.env; set +a
tmux kill-session -t shadow 2>/dev/null
tmux new -d -s shadow "JAR_EXEC_MODE=paper JAR_RPC_URL=\"https://rpc.shyft.to?api_key=$SHYFT_KEY\" JAR_RPS=5 ./target/release/jaringan shadow reports/web/watch_all.json reports/shadow > logs/shadow.log 2>&1"
sleep 10; head -2 logs/shadow.log
cp reports/web/watch_all.json dataset/teams/watch_all.json
git add -A
for v in $(grep -E "^[A-Z_]+=" .env | cut -d= -f2-); do git grep -q --cached -F -- "$v" && { echo SECRET_ABORT; git reset -q; exit 1; }; done
git commit -q -m "Paper executor in the monitor (class A buys, B/C signal-only, dev-family exit), full A/B/C watchlist

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
printf "#!/bin/sh\necho \"\$GITHUB_TOKEN\"\n" > /tmp/askpass.sh; chmod 700 /tmp/askpass.sh
GIT_ASKPASS=/tmp/askpass.sh GIT_TERMINAL_PROMPT=0 git push -q origin main 2>&1 | grep -v -i token | tail -2; rm -f /tmp/askpass.sh
git log --oneline | head -1
echo PAPER_DONE
