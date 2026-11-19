#!/usr/bin/env bash
# T0 検証用：OSC 7501 のテストケースを 1 つずつ送る。
#
#   zellij --layout t0.kdl            # プロジェクトルートで起動
#   bash scripts/t0-cases.sh          # 左下のターミナルペインで実行
#
# 各ケースの送信後に Enter 待ちになるので、その間に probe の表示を確認する。
# probe のログは target/t0-probe.log にも追記される。
set -u

osc() { printf '\033]7501;%s\033\\' "$1"; }
b64() { printf '%s' "$1" | base64 -w0; }

case_no=0
step() {
    case_no=$((case_no + 1))
    printf '\n[%02d] %s\n     body: %s\n' "$case_no" "$1" "$2"
    osc "$2"
    read -r -p "     Enter で次へ（s でスキップ終了）: " ans
    [[ "$ans" == "s" ]] && exit 0
}

echo "pane: ZELLIJ_PANE_ID=${ZELLIJ_PANE_ID:-?}"

# --- キーと値の形式 ---
step "基本形（msg は base64）" "state=working:app=claude:msg=$(b64 'hello working')"
step "msg を平文で送る（base64 必須か）" "state=working:app=claude:msg=plain_text"
step "msg に日本語（UTF-8 を base64）" "state=working:app=claude:msg=$(b64 '日本語メッセージ')"
step "title を平文で送る" "state=working:app=claude:title=plain_title"
step "title を base64 で送る" "state=working:app=claude:title=$(b64 'b64 title')"
step "progress" "state=working:app=claude:progress=42"
step "progress 範囲外（150）" "state=working:app=claude:progress=150"
step "blocked + kind=permission" "state=blocked:kind=permission:app=claude:msg=$(b64 'allow Bash?')"
step "blocked + kind=question" "state=blocked:kind=question:app=claude"
step "done" "state=done:app=claude:msg=$(b64 'finished')"
step "error" "state=error:app=claude:msg=$(b64 'failed')"
step "idle" "state=idle:app=claude"
step "未知の state（foo）" "state=foo:app=claude"
step "app 無し" "state=working"
step "部分更新：前回値が残るか（msg だけ送る）" "msg=$(b64 'only msg')"

# --- 複数レコード ---
step "id=a を追加" "id=a:state=working:app=claude:msg=$(b64 'agent a')"
step "id=b を追加" "id=b:state=blocked:kind=question:app=codex:msg=$(b64 'agent b')"
step "id=a を done に更新" "id=a:state=done:app=claude"
step "id=a だけ clear" "id=a:state=clear"

# --- clear ---
step "ルートを clear（id=b は残るか）" "state=clear"
step "もう一度 working を送ってから全体の clear を確認する準備" "state=working:app=claude"

# --- 終端の違い ---
case_no=$((case_no + 1))
printf '\n[%02d] BEL 終端で送る（ST ではなく \\a）\n' "$case_no"
printf '\033]7501;%s\a' "state=done:app=bel_test"
read -r -p "     Enter で次へ: " _

# --- 頻度 ---
case_no=$((case_no + 1))
printf '\n[%02d] 20 回連続送信（progress 0..95、間隔なし）\n' "$case_no"
for i in $(seq 0 5 95); do osc "state=working:app=burst:progress=$i"; done
read -r -p "     Enter で次へ: " _

case_no=$((case_no + 1))
printf '\n[%02d] 20 回送信（100ms 間隔）\n' "$case_no"
for i in $(seq 0 5 95); do
    osc "state=working:app=burst100:progress=$i"
    sleep 0.1
done
read -r -p "     Enter で次へ: " _

# --- query ---
case_no=$((case_no + 1))
printf '\n[%02d] query（ESC ]7501;? ESC \\）への応答\n' "$case_no"
old=$(stty -g)
stty raw -echo
osc "?"
resp=$(timeout 1 dd bs=1 count=512 2>/dev/null </dev/tty || true)
stty "$old"
printf '     response: %q\n' "$resp"
read -r -p "     Enter で次へ: " _

step "最終状態：working のまま終了（終了後もレコードが残るか確認）" "state=working:app=claude:msg=$(b64 'left over')"
echo "done. シェルに戻った後の probe 表示も確認してください。"
