#!/bin/bash
# Gemini Review Daemon — 定时检查新 PR，调用 Gemini 审查
# 用法: nohup bash scripts/gemini-review-daemon.sh &
# 或 crontab: */5 * * * * /path/to/gemini-review-daemon.sh

REPO="xqicxx/lockit"
APP_ID="3208000"
# 从文件读取 private key
KEY_FILE="$HOME/.config/gemini-review/key.pem"
# 从文件读取 Gemini API key
GEMINI_KEY=$(cat "$HOME/.config/gemini-review/gemini-api-key")
STATE_FILE="$HOME/.config/gemini-review/last_reviewed.json"

# 生成 GitHub App JWT
generate_jwt() {
    local header=$(echo -n '{"alg":"RS256","typ":"JWT"}' | base64 -w 0 | tr '+/' '-_' | tr -d '=')
    local payload=$(echo -n "{\"iat\":$(date -d '30 seconds ago' +%s),\"exp\":$(date -d '10 minutes' +%s),\"iss\":\"$APP_ID\"}" | base64 -w 0 | tr '+/' '-_' | tr -d '=')
    local signature=$(echo -n "$header.$payload" | openssl dgst -sha256 -sign "$KEY_FILE" | base64 -w 0 | tr '+/' '-_' | tr -d '=')
    echo "$header.$payload.$signature"
}

# 获取 installation access token
get_token() {
    local jwt=$(generate_jwt)
    curl -s -H "Authorization: Bearer $jwt" \
         -H "Accept: application/vnd.github.v3+json" \
         "https://api.github.com/app/installations/$(cat $HOME/.config/gemini-review/installation_id)/access_tokens" \
    | python3 -c "import json,sys; print(json.load(sys.stdin)['token'])"
}

# 获取未审查的 PR
get_unreviewed_prs() {
    local token=$1
    curl -s -H "Authorization: token $token" \
         -H "Accept: application/vnd.github.v3+json" \
         "https://api.github.com/repos/$REPO/pulls?state=open&sort=updated&direction=desc&per_page=5" \
    | python3 -c "
import json,sys
prs = json.load(sys.stdin)
state = {}
try:
    with open('$STATE_FILE') as f: state = json.load(f)
except: pass
for pr in prs:
    num = str(pr['number'])
    updated = pr['updated_at']
    if state.get(num) != updated:
        print(f\"{pr['number']}|{pr['title']}|{pr['head']['ref']}\")
"
}

# 调用 Gemini 审查
review_pr() {
    local token=$1 number=$2 title=$3 branch=$4
    echo "Reviewing PR #$number: $title"

    # 获取 diff
    local diff=$(curl -s -H "Authorization: token $token" \
        -H "Accept: application/vnd.github.v3+json" \
        "https://api.github.com/repos/$REPO/pulls/$number" \
        | python3 -c "import json,sys; print(json.load(sys.stdin).get('diff_url',''))" 2>/dev/null)

    if [ -z "$diff" ]; then
        echo "  No diff URL found"
        return
    fi

    local diff_content=$(curl -s -H "Authorization: token $token" "$diff" | head -15000)

    # 调用 Gemini
    local review=$(echo "$diff_content" | gemini -m gemini-3.1-pro-preview -p "你是 Rust + Svelte 代码审查专家。审查以下 PR。
关注：安全性、代码质量、架构。
输出中文，简洁。每个问题标注 🔴严重 / 🟡建议 / 🟢可忽略。
最后结论：可以 Merge / 需要修改。
" 2>&1)

    # 发评论
    local body=$(echo -e "## 🔍 Gemini Code Review\n\n$review" | python3 -c "import json,sys; print(json.dumps(sys.stdin.read())[1:-1])")
    curl -s -X POST \
        -H "Authorization: token $token" \
        -H "Accept: application/vnd.github.v3+json" \
        -d "{\"body\": $body}" \
        "https://api.github.com/repos/$REPO/issues/$number/comments" > /dev/null

    echo "  ✅ Review posted"

    # 更新状态
    local new_state=$(python3 -c "
import json
try:
    with open('$STATE_FILE') as f: d = json.load(f)
except: d = {}
d['$number'] = '$(date -u +%Y-%m-%dT%H:%M:%SZ)'
with open('$STATE_FILE', 'w') as f: json.dump(d, f)
")
}

# ── Main loop ──
mkdir -p "$(dirname $STATE_FILE)"
echo "🤖 Gemini Review Daemon started"
echo "   Repo: $REPO"
echo "   Checking every 5 minutes..."

while true; do
    TOKEN=$(get_token)
    if [ -z "$TOKEN" ]; then
        echo "❌ Failed to get token"
        sleep 300
        continue
    fi

    get_unreviewed_prs "$TOKEN" | while IFS='|' read -r number title branch; do
        review_pr "$TOKEN" "$number" "$title" "$branch"
    done

    sleep 300  # 5 分钟
done
