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
| `badge_items` | 要素の一覧（カンマか空白で区切る） | `title tab_name`（`nerd_font` が `true` なら `app_icon title tab_name`） | 状態バッジの中に表示する要素（「表示の形」を参照） |
| `text_items` | 要素の一覧（カンマか空白で区切る） | 空 | 状態バッジの外に表示する要素（「表示の形」を参照） |
| `msg_mode` | `none` / `attention` / `all` | `attention` | msg の表示モード。表示しない、`Blocked` / `Error` のエージェントだけ、すべてのエージェント |
| `max_agent_width` | `0` か、20 以上の整数 | `50` | 1 エージェント分の表示幅（桁数）の上限。状態アイコンから msg までを含む。`0` なら上限なし |
| `state_icon_style` | `symbol` / `alphabet` / `emoji` / `nerd_font` | `symbol`（`nerd_font` が `true` なら `nerd_font`） | 状態アイコンの種類。記号、アルファベット 1 文字、絵文字、Nerd Font |
| `badge_shape` | `square` / `slash` / `arrow` / `round` | `square`（`nerd_font` が `true` なら `slash`） | 状態バッジの端の形。四角、斜め、矢印、半円。`state_icon_style "emoji"` では効かない |
| `highlight_attention` | `true` / `false` | `false` | `Blocked` / `Error` のエージェントを、msg までの全体を状態の色で塗って強調するか |
| `nerd_font` | `true` / `false` | `false` | Nerd Font を使う既定値に切り替えるか。変わるのは `state_icon_style` / `badge_items` / `badge_shape` の既定値だけで、指定した値はそのまま使う（`badge_items` を指定したときは `app_icon` を足さない） |
| `nerd_font_icon_padding` | `true` / `false` | `false` | Nerd Font のアイコン（状態アイコンと `app` のアイコン）の右隣に空白を置くか |

不正な値を指定したキーは既定値に戻り、表示するエージェントがいないときに、プラグインの行へ警告を 1 つだけ表示します（すべての警告は Zellij のログに出ます）。知らないキーは警告なしで無視するので、設定が効かないときはキーの名前を確かめてください。

### 表示の形

1 エージェント分は次の形で表示します。`[ ]` は状態バッジ（状態の色で背景を塗った部分）で、`!` は状態アイコンです。進捗と msg は、あるときだけ表示します。

```
[! 42% badge_items] text_items msg
```

`badge_items` / `text_items` に使える要素は次のとおりです。書いた要素だけを、書いた順に表示します。

| 要素 | 表示するもの |
|---|---|
| `title` | エージェントが送る `title` |
| `tab_name` | ペインがあるタブの名前 |
| `app_name` | `app`（アプリの名前） |
| `app_icon` | `app` のアイコン。アイコンがあるのは Claude Code だけ |
| `cwd` | カレントディレクトリ（cwd）の末尾のディレクトリ名。括弧で囲む |

`Blocked`、タブ名 `work`、`app` が `claude-code`、cwd が `repo`、`title` なしのときの例です。

```
[! work] msg                       既定
[! work claude-code] msg           badge_items "tab_name app_name"
[! work] claude-code (repo): msg   badge_items "tab_name"、text_items "app_name cwd"
[!] work: msg                      badge_items ""、text_items "tab_name"
[! work: msg]                      highlight_attention "true"（Blocked / Error のエージェントだけ）
❗ work: msg                       state_icon_style "emoji"（状態バッジにならない）
```

- 状態・`app`・`title`・msg・進捗は、エージェントが OSC 7501 で送ります。状態は `Idle`（待機中）・`Working`（作業中）・`Blocked`（許可や質問などの操作待ち）・`Done`（完了）・`Error`（エラー）のどれかで、`app` はアプリの名前（Claude Code なら `claude-code`）です。Claude Code は `title` を送りません。
- 値のない要素は、前の空白や括弧も含めて省きます。
- `title` / `tab_name` / `app_name` / `cwd` のうち、表示できる要素が 1 つもないエージェントは、代わりに `#<ペインの id>` を `badge_items` の末尾（`badge_items` が空なら `text_items` の末尾）に表示します。一覧にない要素は、値があっても使いません。
- 同じ要素が両方のキーにあるときは、`badge_items` だけに表示します。`badge_items` を指定していなければ既定値と比べるので、たとえば `text_items "tab_name"` だけを指定しても、タブ名はバッジの中に表示されます。外に出すときは、`badge_items` も指定してください。
- 幅が足りないとき（`max_agent_width` を超えるときも）は、一覧の順番に関係なく、msg → `cwd` → `app_name` → `tab_name` → `title` の順に、切り詰めるか省きます。ただし、各エージェントで最後に残る要素は省かず、最後にまとめて切り詰めます。それでも足りなければ、状態アイコンと進捗だけにします。切り詰めたところは `~` になります。
- 状態の色はテーマの色を使うので、背景にしたときの見え方はテーマによって変わります。

### cwd

要素の `cwd` で表示するのは、ペインのルートプロセス（通常はシェル）のカレントディレクトリです。エージェントが中で別のディレクトリ（worktree など）に移っても追従せず、SSH 先やサンドボックス内で動くエージェントでは手元のシェルのディレクトリになります。worktree ごとに見分けたいときは、そのディレクトリに `cd` してからエージェントを起動してください。

### Nerd Font

- 次のどれかを使うときは、端末のフォントが [Nerd Fonts](https://www.nerdfonts.com/) である必要があります：`state_icon_style "nerd_font"`、要素の `app_icon`、`square` 以外の `badge_shape`（`nerd_font "true"` では、既定でどれも使います）。
- Nerd Font の文字は 1 桁として幅を計算します。端末で East Asian Width が Ambiguous の文字を 2 桁で表示する設定にしていると、表示やクリックの位置がずれます。
- フォントと端末の組み合わせによっては、アイコンの右が切れたり隣の文字と重なったりします（`badge_items` が空のときや、`app_icon` をバッジの末尾に置いたとき、幅が足りず状態アイコンだけになったときに起きやすいです）。そのときは `nerd_font_icon_padding "true"` を指定してください。

## 開発

- ビルドターゲットは `.cargo/config.toml` で `wasm32-wasip1` に固定しています。ユニットテストはホストのターゲットを指定して実行します。

  ```sh
  cargo build
  cargo test --target x86_64-unknown-linux-gnu
  ```

- 動作確認は `zellij --layout dev.kdl` で行います。中央のペインで `scripts/osc7501-demo.sh` を実行すると、下から 2 行目に表示されます。再ビルドした後は、上の `bash` ペインで Enter を押すとタブごと開き直して読み込み直します。
- OSC 7501 のレコードは、送信元のプログラムが終わってシェルに戻ると約 2 秒で Zellij が消します。手動で送るときは送信元を終了させないでください（`scripts/osc7501.sh` は既定で Enter 待ちになります）。
- OSC 7501 のフォーマットと Zellij 側の挙動は `doc/osc7501-findings.md` にまとめています。
