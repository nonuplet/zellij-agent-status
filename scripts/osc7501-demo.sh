#!/usr/bin/env bash
# 表示確認用：OSC 7501 のルートレコードを、状態を変えながら送り続ける。Ctrl-C で終了する。
#
#   scripts/osc7501-demo.sh            # 2 秒ごとに切り替える
#   scripts/osc7501-demo.sh 0.5 codex  # 間隔と app 名を指定する
#
# 複数エージェントの並びを見たいときは、ペインごとに app 名を変えて実行する。
# プラグインの設定で app_filter を指定していると、既定の app 名（demo）は表示されない。
# app_filter を外すか、app_filter に含まれる app 名を指定する。
set -euo pipefail

interval="${1:-2}"
app="${2:-demo}"

osc() { printf '\033]7501;%s\033\\' "$1"; }
b64() { printf '%s' "$1" | base64 | tr -d '\n'; }

# 終了時にレコードを消す（何もしなくても Zellij が約 2 秒後に消すが、すぐに消したい）
trap 'osc "state=clear"; printf "\ncleared\n"; exit 0' INT TERM

long_msg='とても長いメッセージ：表示幅に収まらないときの切り詰めを確認するための文字列です。The quick brown fox jumps over the lazy dog.'

# 説明<TAB>body
cases=(
    "idle	state=idle:app=$app"
    "working（msg なし）	state=working:app=$app"
    "working + progress 30	state=working:app=$app:progress=30:msg=$(b64 'Bash: cargo build')"
    "working + progress 80	state=working:app=$app:progress=80:msg=$(b64 'Edit: src/main.rs')"
    "blocked / permission	state=blocked:kind=permission:app=$app:msg=$(b64 'Bash: rm -rf target')"
    "blocked / question	state=blocked:kind=question:app=$app:msg=$(b64 'どちらの方針で進めますか？')"
    "blocked / auth	state=blocked:kind=auth:app=$app"
    "working（長い msg）	state=working:app=$app:msg=$(b64 "$long_msg")"
    "done	state=done:app=$app:title=$(b64 'zellij-agent-status'):msg=$(b64 '実装が完了しました')"
    "error	state=error:app=$app:msg=$(b64 'API Error: Rate limit reached')"
)

printf 'pane: ZELLIJ_PANE_ID=%s  interval=%ss  app=%s\n' "${ZELLIJ_PANE_ID:-?}" "$interval" "$app"
while :; do
    for c in "${cases[@]}"; do
        printf '%-28s %s\n' "${c%%	*}" "${c#*	}"
        osc "${c#*	}"
        sleep "$interval"
    done
done
