#!/bin/bash
# Back up the project to a NEW PRIVATE GitHub repo. Secrets (.env) are never committed; a scan aborts the push
# if any key value appears in a tracked file.
cd /root/jaringan
set -a; . ./.env; set +a
REPO=jaringan-dev-solana; OWNER=$(curl -s -H "Authorization: Bearer $GITHUB_TOKEN" https://api.github.com/user | jq -r .login)
pgrep -f "[g]h_prepare.sh" > /dev/null && while pgrep -f "[g]h_prepare.sh" > /dev/null; do sleep 10; done
[ -d dataset/launches ] && [ "$(ls dataset/launches | wc -l)" -ge 10 ] || ./gh_prepare.sh
cat > .gitignore <<G
.env
*.env
target/
data/
logs/
/tmp/
*.tmp
G
[ -d .git ] || git init -q -b main
git config user.name "$OWNER"; git config user.email "$OWNER@users.noreply.github.com"
git add -A
# secret scan: every value in .env must be absent from tracked files
for v in $(grep -E "^[A-Z_]+=" .env | cut -d= -f2-); do
  if git grep -q -F -- "$v" $(git ls-files) 2>/dev/null; then echo "SECRET FOUND in tracked files - abort"; git reset -q; exit 1; fi
done
git commit -q -m "Dev-network research system: Rust pipeline, team reputation, coin and wallet datasets

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" || true
curl -s -o /dev/null -w "create repo: %{http_code}\n" -H "Authorization: Bearer $GITHUB_TOKEN" -H "Accept: application/vnd.github+json" https://api.github.com/user/repos -d "{\"name\":\"$REPO\",\"private\":true,\"description\":\"Solana dev-network research (private)\"}"
printf "#!/bin/sh\necho \"\$GITHUB_TOKEN\"\n" > /tmp/askpass.sh; chmod 700 /tmp/askpass.sh
git remote remove origin 2>/dev/null; git remote add origin https://x-access-token@github.com/$OWNER/$REPO.git
GIT_ASKPASS=/tmp/askpass.sh GIT_TERMINAL_PROMPT=0 git -c http.postBuffer=524288000 push -u origin main 2>&1 | grep -v -i token | tail -5
rm -f /tmp/askpass.sh
curl -s -H "Authorization: Bearer $GITHUB_TOKEN" https://api.github.com/repos/$OWNER/$REPO | jq -r "\"repo: \" + .html_url + \" private=\" + (.private|tostring) + \" size_kb=\" + (.size|tostring)"
echo GH_DONE
