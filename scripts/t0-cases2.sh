#!/usr/bin/env bash
# T0 追加検証：t0-cases.sh で未確定だった点を確かめる。
#
#   zellij --layout t0.kdl            # プロジェクトルートで起動
#   bash scripts/t0-cases2.sh         # 左下のターミナルペインで実行
#
# 各ケースの開始時に、probe のログ（target/t0-probe.log）へ区切り行を直接追記する。
# probe 側は 0.5 秒ごとに get_pane_info をポーリングし、変化があれば POLL 行を出す。
set -u

LOG="$(cd "$(dirname "$0")/.." && pwd)/target/t0-probe.log"

osc() { printf '\033]7501;%s\033\\' "$1"; }
b64() { printf '%s' "$1" | base64 -w0; }

case_no=0
mark() {
    case_no=$((case_no + 1))
    printf '\n[%02d] %s\n' "$case_no" "$1"
    printf '======== [%02d] %s ========\n' "$case_no" "$1" >>"$LOG"
}
wait_enter() {
    read -r -p "     Enter で次へ（s でスキップ終了）: " ans
    [[ "$ans" == "s" ]] && exit 0
}
step() {
    mark "$1"
    printf '     body: %s\n' "$2"
    osc "$2"
    wait_enter
}

printf '\n######## t0-cases2 start (ZELLIJ_PANE_ID=%s) ########\n' "${ZELLIJ_PANE_ID:-?}" >>"$LOG"

# --- 準備 ---
step "前回の状態をクリア" "state=clear"
step "ベースライン：ルートを working に" "state=working:app=claude:msg=$(b64 'base')"

# --- 未知の state ---
step "未知の state（working のまま残るか、idle になるか）" "state=foo:app=claude:msg=$(b64 'foo')"
step "ベースラインに戻す" "state=working:app=claude:msg=$(b64 'base')"

# --- 何も送らないときのイベント頻度 ---
mark "12 秒間なにも送らない（WINDOW 行で noop の増え方を見る。操作不要）"
sleep 12
wait_enter

# --- id 付きレコード（ポーリングで見えるか） ---
step "id=a を追加（ルートは working のまま）" "id=a:state=working:app=claude:msg=$(b64 'agent a')"
step "id=a を done に" "id=a:state=done:app=claude:msg=$(b64 'agent a done')"
step "id=a の msg だけ変える" "id=a:state=done:app=claude:msg=$(b64 'agent a changed')"
step "id=a だけ clear" "id=a:state=clear"
step "id=b を blocked で追加（attention が NeedsYou になる）" "id=b:state=blocked:kind=question:app=codex:msg=$(b64 'agent b')"
step "id=b だけ clear（attention がルートの working に戻るか）" "id=b:state=clear"

# --- クリアのきっかけ ---
step "ベースラインに戻す" "state=working:app=claude:msg=$(b64 'base')"

mark "子プロセスから送信し、子プロセスだけ終了する（スクリプトは継続。5 秒待つ）"
bash -c "printf '\033]7501;%s\033\\\\' 'state=working:app=child:msg=$(b64 'from child')'; sleep 1"
echo "     子プロセス終了。5 秒待機中..."
sleep 5
wait_enter

mark "hooks 想定：stdout/stderr を捨てた子プロセスが /dev/tty に書く（5 秒待つ）"
bash -c "printf '\033]7501;%s\033\\\\' 'state=blocked:kind=permission:app=hook:msg=$(b64 'from hook')' > /dev/tty" >/dev/null 2>&1 </dev/null
echo "     子プロセス終了。5 秒待機中..."
sleep 5
wait_enter

# --- フォーカスと attention ---
mark "フォーカス外での done：5 秒後に done を送るので、その間に右の probe ペインをクリックしてフォーカスを移す"
echo "     5 秒後に送信します。いまのうちに右のペインへフォーカスを移してください。"
sleep 5
osc "state=done:app=claude:msg=$(b64 'done while unfocused')"
echo "     送信しました。probe で attention を確認したら、このペインに戻って Enter。"
wait_enter

# --- 終了時のクリア ---
step "working のままスクリプトを終了する" "state=working:app=claude:msg=$(b64 'left over')"

cat <<EOF

スクリプトは終了しました。続けて、シェルのプロンプトで次のコマンドを順に実行してください。
（各コマンドの後、数秒待ってから次へ。1 行目の echo はログへの区切り行です）

  echo '======== [M1] プロンプトから直接送信 ========' >> target/t0-probe.log; printf '\033]7501;state=working:app=prompt\033\\\\'
  echo '======== [M2] 外部コマンドを実行 ========' >> target/t0-probe.log; env true
  echo '======== [M3] 再送してから sleep 3 ========' >> target/t0-probe.log; printf '\033]7501;state=working:app=prompt2\033\\\\'; sleep 3
  echo '======== [M4] 終了 ========' >> target/t0-probe.log
EOF
