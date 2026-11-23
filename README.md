# zellij-agent-status

Zellij のステータスバーにAIエージェントのステータスを表示するためのプラグインです。

OSC 7501 を読んでステータスの情報を取得します。Zellij 0.46 以降が必要です。

## 使い方

- Claude Code（最新版）は自分で OSC 7501 を送るので、hooks などの設定なしで表示されます。エージェントごとの挙動は `doc/agent-behaviors.md` を参照してください。
- 手動で送るときは `scripts/osc7501.sh` を使います。
- 初回の起動時は、プラグインのペインに permission（`ReadApplicationState`・`ChangeApplicationState`・`InterceptInput`）の確認が出ます。1 行のペインなので全文は読めませんが、ペインをクリックしてフォーカスし、許可してください。許可するまでエージェントの状態は表示されません。
- 並んでいるエージェントをクリックすると、そのエージェントのペインにフォーカスが移ります。ペインが別のタブにあれば、そのタブに切り替わります。
- キー操作でも選べます（キーバインドの設定が必要です。次の「選択モードのキー」を参照）。選択モードに入り、`←` / `→`（`h` / `l`）で選んで `Enter` でそのペインにフォーカスします。`Esc`（`Ctrl c`）で何もせずに抜けます。選択モード中に行をクリックしたり、ほかのペインやタブにフォーカスが移ったりしたときも抜けます。

### 選択モードのキー

プラグインはキーを登録しません。`config.kdl` の `keybinds` に、パイプ（`MessagePlugin`）で `zellij-agent-status::select` を送るキーを書いてください。次の例では `Ctrl t`（tab モード）→ `a` で選択モードに入ります（tab モードの `a` は、default / unlock-first のどちらのプリセットでも使われていません）。

```kdl
keybinds {
    tab {
        bind "a" {
            MessagePlugin { name "zellij-agent-status::select"; }
            SwitchToMode "Normal"; // unlock-first のプリセットなら "Locked"
        }
    }
}
```

パイプは `name` だけを指定し、`plugin` に URL は書かないでください。URL を書くと、設定値の違いなどで別のインスタンスが起動することがあります。

## 設定

レイアウトの plugin ブロックに書きます。どのキーも省略できます。次の例は、Nerd Font を使い、操作待ちやエラーを目立たせる設定です。

```kdl
pane size=1 borderless=true {
    plugin location="file:/path/to/zellij-agent-status.wasm" {
        nerd_font "true"
        blocked_first "true"
        highlight_attention "true"
    }
}
```

| キー | 値 | 既定値 | 内容 |
|---|---|---|---|
| `app_filter` | `app` の一覧（カンマか空白で区切る） | 空 | 表示するエージェントを `app` で絞り込む（完全一致。`app` のないエージェントは表示しない）。空なら絞り込まない |
| `blocked_first` | `true` / `false` | `false` | `Blocked` のエージェントを先頭に並べるか。`false` なら常にタブ・ペインの順 |
| `use_tab_name` | `true` / `false` | `true` | タブ名を名前の候補にするか |
| `use_app_name` | `true` / `false` | `true` | `app` を名前の候補にするか |
| `show_cwd` | `true` / `false` | `false` | 名前の後ろに、カレントディレクトリ（cwd）の末尾のディレクトリ名を表示するか |
| `msg_mode` | `none` / `attention` / `all` | `attention` | msg の表示モード。表示しない、`Blocked` / `Error` のエージェントだけ、すべてのエージェント |
| `max_agent_width` | `0` か、18 以上の整数 | `50` | 1 エージェント分の表示幅（桁数）の上限。状態アイコンから msg までを含む。`0` なら上限なし |
| `state_icon_style` | `symbol` / `alphabet` / `emoji` / `nerd_font` | `symbol`（`nerd_font` が `true` なら `nerd_font`） | 状態アイコンの種類。記号、アルファベット 1 文字、絵文字、Nerd Font |
| `show_app_icon` | `true` / `false` | `false`（`nerd_font` が `true` なら `true`） | 名前の前に `app` のアイコンを表示するか。アイコンがあるのは Claude Code だけ |
| `badge_shape` | `square` / `slash` / `arrow` / `round` | `square`（`nerd_font` が `true` なら `slash`） | 状態バッジの端の形。四角、斜め、矢印、半円。`state_icon_style "emoji"` では効かない |
| `name_in_badge` | `true` / `false` | `true` | 状態バッジに名前（`app` のアイコンと cwd を含む）も入れるか。`state_icon_style "emoji"` では効かない |
| `highlight_attention` | `true` / `false` | `false` | `Blocked` / `Error` のエージェントを、msg までの全体を状態の色で塗って強調するか |
| `nerd_font` | `true` / `false` | `false` | Nerd Font を使う既定値に切り替えるか。変わるのは `state_icon_style` / `show_app_icon` / `badge_shape` の既定値だけで、指定した値はそのまま使う |
| `nerd_font_icon_padding` | `true` / `false` | `false` | Nerd Font のアイコン（状態アイコンと `app` のアイコン）の右隣に空白を置くか |

不正な値を指定したキーは既定値に戻り、表示するエージェントがいないときに、プラグインの行へ警告を 1 つだけ表示します（すべての警告は Zellij のログに出ます）。知らないキーは警告なしで無視するので、設定が効かないときはキーの名前を確かめてください。

### 表示の形

1 エージェント分は次の形で表示します。`[ ]` は状態バッジ（状態の色で背景を塗った部分）で、`!` は状態アイコンです。進捗・cwd・msg は、あるときだけ表示します。

```
[! 42% 名前 (cwd)] msg     既定
[! 42%] 名前 (cwd): msg    name_in_badge "false"
[! 42% 名前 (cwd): msg]    highlight_attention "true"（Blocked / Error のエージェントだけ）
❗ 42% 名前 (cwd): msg     state_icon_style "emoji"（状態バッジにならない）
```

- 状態・`app`・`title`・msg・進捗は、エージェントが OSC 7501 で送ります。状態は `Idle`（待機中）・`Working`（作業中）・`Blocked`（許可や質問などの操作待ち）・`Done`（完了）・`Error`（エラー）のどれかで、`app` はアプリの名前（Claude Code なら `claude-code`）です。
- 名前は、`title` → タブ名 → `app` → `#<ペインの id>` の順に見て、最初に見つかったものです。`use_tab_name` / `use_app_name` が `false` なら、タブ名 / `app` を飛ばします。Claude Code は `title` を送らないので、両方 `false` にすると `#<ペインの id>` になります。
- 幅が足りないとき（`max_agent_width` を超えるときも）は、msg → cwd → 名前の順に、切り詰めるか省きます。切り詰めたところは `~` になります。
- 状態の色はテーマの色を使うので、背景にしたときの見え方はテーマによって変わります。

### cwd

`show_cwd` で表示するのは、ペインのルートプロセス（通常はシェル）のカレントディレクトリです。エージェントが中で別のディレクトリ（worktree など）に移っても追従せず、SSH 先やサンドボックス内で動くエージェントでは手元のシェルのディレクトリになります。worktree ごとに見分けたいときは、そのディレクトリに `cd` してからエージェントを起動してください。

### Nerd Font

- 次のどれかを使うときは、端末のフォントが [Nerd Fonts](https://www.nerdfonts.com/) である必要があります：`state_icon_style "nerd_font"`、`show_app_icon "true"`、`square` 以外の `badge_shape`（`nerd_font "true"` では、既定でどれも使います）。
- Nerd Font の文字は 1 桁として幅を計算します。端末で East Asian Width が Ambiguous の文字を 2 桁で表示する設定にしていると、表示やクリックの位置がずれます。
- フォントと端末の組み合わせによっては、アイコンの右が切れたり隣の文字と重なったりします（`name_in_badge` が `false` のときや、幅が足りず状態アイコンだけになったときに起きやすいです）。そのときは `nerd_font_icon_padding "true"` を指定してください。

## 開発

- ビルドターゲットは `.cargo/config.toml` で `wasm32-wasip1` に固定しています。ユニットテストはホストのターゲットを指定して実行します。

  ```sh
  cargo build
  cargo test --target x86_64-unknown-linux-gnu
  ```

- 動作確認は `zellij --layout dev.kdl` で行います。中央のペインで `scripts/osc7501-demo.sh` を実行すると、下から 2 行目に表示されます。再ビルドした後は、上の `bash` ペインで Enter を押すとタブごと開き直して読み込み直します。
- OSC 7501 のレコードは、送信元のプログラムが終わってシェルに戻ると約 2 秒で Zellij が消します。手動で送るときは送信元を終了させないでください（`scripts/osc7501.sh` は既定で Enter 待ちになります）。
- OSC 7501 のフォーマットと Zellij 側の挙動は `doc/osc7501-findings.md` にまとめています。
