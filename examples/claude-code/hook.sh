#!/usr/bin/env bash
# Claude Code の hooks から OSC 7501 を送り、Zellij にエージェントの状態を伝える。
# 設定例は同じディレクトリの settings.json。jq が必要。
#
# hooks の stdout は端末につながっていない（JSON の応答として読まれる）ので、/dev/tty に書く。
# hook が終わってもレコードは消えない。消えるのは Claude Code 本体が終わってシェルに戻ったとき。
#
# イベントと状態の対応：
#   SessionStart                    → idle
#   UserPromptSubmit                → working（msg：プロンプトの 1 行目）
#   PreToolUse / PostToolUse(Failure) → working（msg：ツール名）。許可の後に blocked から戻すのにも使う
#   PreToolUse(AskUserQuestion)     → blocked / question
#   PermissionRequest               → blocked / permission（msg：ツール名と内容）
#   Notification(elicitation_dialog) → blocked / question
#   Stop                            → done（msg：最後の応答の 1 行目）
#   StopFailure                     → error（msg：エラー内容）
#   SessionEnd                      → clear
#
# 制約：Esc での中断では Stop が呼ばれないので、working（または blocked）のまま残る。
#       次にプロンプトを送ると working に戻る。
#
# 環境変数 OSC7501_TTY で書き込み先を変えられる（動作確認用。既定は /dev/tty）。

# Zellij の外では何もしない
[[ -n "${ZELLIJ:-}" ]] || exit 0

if ! command -v jq >/dev/null 2>&1; then
    echo 'zellij-agent-status hook: jq が見つからない' >&2
    exit 1
fi

input="$(cat)"

# 1 行目だけを最大 120 文字にする（Zellij 側でも制御文字の除去と 2048 バイトでの切り詰めがある）
jq_first_line='def first_line: (. // "") | tostring | split("\n") | map(select(test("\\S"))) | (.[0] // "") | .[0:120];'

# 出力：state・kind・msg を 1 行ずつ。送らないイベントなら何も出さない
# shellcheck disable=SC2016
decision="$(printf '%s' "$input" | jq -r "$jq_first_line"'
    def tool_summary:
        .tool_name + (
            (.tool_input.command // .tool_input.file_path // .tool_input.url // .tool_input.pattern // null)
            | if . == null then "" else ": " + (. | first_line) end
        );
    def out($state; $kind; $msg): $state, $kind, ($msg | first_line);
    if .hook_event_name == "SessionStart" then out("idle"; ""; "")
    elif .hook_event_name == "UserPromptSubmit" then out("working"; ""; .prompt)
    elif .hook_event_name == "PreToolUse" and .tool_name == "AskUserQuestion" then
        out("blocked"; "question"; .tool_input.questions[0].question)
    elif .hook_event_name == "PreToolUse" or .hook_event_name == "PostToolUse"
        or .hook_event_name == "PostToolUseFailure" then out("working"; ""; .tool_name)
    elif .hook_event_name == "PermissionRequest" then out("blocked"; "permission"; tool_summary)
    elif .hook_event_name == "Notification" and .notification_type == "elicitation_dialog" then
        out("blocked"; "question"; .message)
    elif .hook_event_name == "Stop" then out("done"; ""; .last_assistant_message)
    elif .hook_event_name == "StopFailure" then
        out("error"; ""; (.last_assistant_message // .error))
    elif .hook_event_name == "SessionEnd" then out("clear"; ""; "")
    else empty end
' 2>/dev/null)" || exit 0
[[ -n "$decision" ]] || exit 0

{
    read -r state
    read -r kind
    read -r msg
} <<<"$decision"
# タイトルは作業ディレクトリ名
title="$(printf '%s' "$input" | jq -r '(.cwd // "") | split("/") | map(select(. != "")) | last // ""' 2>/dev/null)"

b64() { printf '%s' "$1" | base64 | tr -d '\n'; }

body="state=$state"
if [[ "$state" != clear ]]; then
    body+=":app=claude"
    [[ -z "$kind" ]] || body+=":kind=$kind"
    [[ -z "$title" ]] || body+=":title=$(b64 "$title")"
    [[ -z "$msg" ]] || body+=":msg=$(b64 "$msg")"
fi

# 端末がない場合（-p での実行など）は黙って終わる。Claude Code の動作を妨げないよう常に 0 で終える。
{ printf '\033]7501;%s\033\\' "$body" >"${OSC7501_TTY:-/dev/tty}"; } 2>/dev/null
exit 0
