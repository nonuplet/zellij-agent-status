#!/usr/bin/env bash
# OSC 7501 を 1 件送る手動テスト用スクリプト。Zellij のターミナルペインで実行する。
#
#   scripts/osc7501.sh working -a claude -m 'ビルド中' -p 40
#   scripts/osc7501.sh blocked -k permission -a claude -m 'Bash: rm -rf target'
#   scripts/osc7501.sh clear
#
# 送信元のプログラムが終わってシェルに戻ると、Zellij は約 2 秒でそのペインのレコードを消す。
# そのため既定では送信後に Enter 待ちでとどまり、レコードを残す。-n を付けると待たずに終わる。
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: osc7501.sh [options] <state>

  state   idle | working | done | blocked | error | clear

Options:
  -a APP       送信元のアプリ名（例：claude）
  -k KIND      blocked の理由：permission | question | auth
  -p PROGRESS  進捗（0〜100）
  -t TITLE     タイトル（base64 にして送る）
  -m MSG       メッセージ（base64 にして送る）
  -i ID        レコードの id（付けると別レコードになる。プラグインは id なしのルートレコードだけを扱う）
  -n           送信後に待たずに終了する（シェルに戻るとレコードは約 2 秒で消える）
  -h           このヘルプを表示する
EOF
}

die() {
    printf 'osc7501.sh: %s\n' "$1" >&2
    exit 2
}

b64() { printf '%s' "$1" | base64 | tr -d '\n'; }

# app と id は平文で送るので、区切り文字（: ; =）と制御文字を含む値は弾く。
check_plain() {
    [[ -n "$2" && "$2" != *[:\;=[:cntrl:]]* ]] || die "$1 に空文字・':' ';' '='・制御文字は使えない: $2"
}

app='' kind='' progress='' title='' msg='' id='' hold=1
positional=()
# getopts は最初の非オプション引数で止まるので使わない（state の後ろにもオプションを書けるようにする）
need_arg() { (($# >= 2)) || die "$1 には値が必要"; }
while (($# > 0)); do
    case "$1" in
    -a | -k | -p | -t | -m | -i)
        need_arg "$@"
        case "$1" in
        -a) app="$2" ;;
        -k) kind="$2" ;;
        -p) progress="$2" ;;
        -t) title="$2" ;;
        -m) msg="$2" ;;
        -i) id="$2" ;;
        esac
        shift 2
        ;;
    -n)
        hold=0
        shift
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    --)
        shift
        positional+=("$@")
        break
        ;;
    -*) die "未知のオプション: $1" ;;
    *)
        positional+=("$1")
        shift
        ;;
    esac
done
[[ ${#positional[@]} -eq 1 ]] || {
    usage >&2
    exit 2
}
state="${positional[0]}"

# 不正な値が 1 つでもあると Zellij はシーケンスごと捨てるので、送る前に確かめる。
case "$state" in
idle | working | done | blocked | error | clear) ;;
*) die "未知の state: $state" ;;
esac
case "$kind" in
'' | permission | question | auth) ;;
*) die "未知の kind: $kind" ;;
esac
if [[ -n "$progress" ]]; then
    [[ "$progress" =~ ^[0-9]+$ ]] && ((10#$progress <= 100)) || die "progress は 0〜100 の整数: $progress"
fi
[[ -z "$app" ]] || check_plain app "$app"
[[ -z "$id" ]] || check_plain id "$id"

body=''
[[ -z "$id" ]] || body+="id=$id:"
body+="state=$state"
[[ -z "$kind" ]] || body+=":kind=$kind"
[[ -z "$progress" ]] || body+=":progress=$((10#$progress))"
[[ -z "$app" ]] || body+=":app=$app"
[[ -z "$title" ]] || body+=":title=$(b64 "$title")"
[[ -z "$msg" ]] || body+=":msg=$(b64 "$msg")"

[[ -n "${ZELLIJ:-}" ]] || printf 'osc7501.sh: Zellij の外で実行している（ZELLIJ が未設定）\n' >&2

printf '\033]7501;%s\033\\' "$body"

if ((hold)) && [[ -t 0 ]]; then
    printf 'sent: %s\n' "$body"
    read -r -p 'Enter で終了（終了後、レコードは約 2 秒で消える）: ' _
fi
