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

レイアウトの plugin ブロックに書きます。

```kdl
pane size=1 borderless=true {
    plugin location="file:/path/to/zellij-agent-status.wasm" {
        icon_style "alphabet"
        apps "claude-code"
        show_app_name "false"
        show_tab_name "false"
        blocked_first "true"
        max_length 40
        show_cwd "true"
        nerd_font "true"
        badge_shape "round"
        nerd_font_icon_padding "true"
        highlight_attention "true"
        badge_name "false"
        msg_mode "all"
    }
}
```

| キー | 既定値 | 内容 |
|---|---|---|
| `icon_style` | `symbol`（`nerd_font` が `true` なら `nerd`） | アイコンを記号（`symbol`）、アルファベット 1 文字（`alphabet`）、絵文字（`emoji`）、Nerd Font（`nerd`）のどれにするか。絵文字は状態の色で塗られず、バッジにもならない |
| `apps` | （空） | 表示する `app` の一覧。カンマか空白で区切る。空なら絞り込まない。指定したときは `app` のないレコードを表示しない |
| `show_app_name` | `true` | `app` を名前の候補にするか（`true` / `false`）。`apps` の絞り込みには影響しない |
| `show_tab_name` | `true` | タブ名を名前の候補にするか（`true` / `false`）。タブバーなどでタブ名がすでに見えているときや、1 タブに 1 エージェントしか置かないときは `false` にすると重複しない |
| `blocked_first` | `false` | 操作待ち（`Blocked`）のエージェントを先頭に並べるか（`true` / `false`）。`false` なら常にタブ・ペインの順。`Blocked` 同士はタブ・ペインの順で、`Blocked` でなくなると元の位置に戻る |
| `max_length` | `50` | 1 エージェント分（アイコン・進捗を含む）の表示幅の上限。18 以上の整数。超える分は msg から切り詰める。`0` なら上限なし |
| `show_cwd` | `false` | 名前の後ろにカレントディレクトリの末尾のディレクトリ名を `名前 (dir)` の形で表示するか（`true` / `false`）。幅が足りないとき（`max_length` を超えるときも）は msg の次に省く |
| `nerd_font` | `false` | Nerd Font モード（`true` / `false`）。バッジの端を `badge_shape` の形にし、名前の前にエージェント（`app`）のアイコンを付け（今は Claude Code のみ）、`icon_style` の既定値を `nerd` にする。`icon_style` を指定すればそちらが優先される |
| `badge_shape` | `slash` | Nerd Font モードの状態バッジの端の形。斜め（`slash`）、矢印（`arrow`）、半円（`round`）のどれか。`nerd_font` が `false` なら使わない |
| `nerd_font_icon_padding` | `false` | Nerd Font のアイコン（状態アイコンと `app` のアイコン）の右隣に空白を置くか（`true` / `false`）。アイコンの右が切れたり隣と重なったりするときに使う（`badge_name` が `true` なら、たいていは要らない） |
| `highlight_attention` | `false` | 注意が必要な状態（`Blocked` / `Error`）のエージェントを、1 つ分全体を状態の色の背景で表示して強調するか（`true` / `false`）。絵文字アイコンのときも強調する |
| `badge_name` | `true` | 状態バッジの中に、名前（と Nerd Font モードの `app` のアイコン、`show_cwd` の cwd）も入れるか（`true` / `false`）。`false` なら状態アイコンと進捗だけをバッジにする。`true` ならバッジの端が名前と msg の境目になるので、間の `: ` を空白 1 つにする（強調したエージェントは msg までバッジなので `: ` のまま）。絵文字アイコンはバッジにならないので効かない |
| `msg_mode` | `attention` | msg を表示するエージェント。表示しない（`none`）、注意が必要な状態（`Blocked` / `Error`、`highlight_attention` と同じ）のエージェントだけ（`attention`）、すべて（`all`）のどれか。msg を表示しないエージェントの分の幅は、ほかの部分に回す |
エージェントの名前は `title` → タブ名 → `app` → `#<pane_id>` の順で決まり、`show_tab_name` / `show_app_name` が `false` ならタブ名 / `app` を飛ばします。Claude Code は `title` を送らないので、両方 `false` にすると名前は `#<pane_id>` になります。

状態アイコンから名前（と `app` のアイコン・cwd）までは、状態の色を背景にしたバッジとして表示します（Nerd Font モードでは端を `badge_shape` の形にします）。名前を省くほど幅が足りないときは、状態アイコンと進捗だけのバッジになります。状態の色はテーマの前景色向けの色を使っているので、背景にしたときの見え方はテーマ次第です。

`nerd_font` と `icon_style "nerd"` には、端末のフォントが [Nerd Fonts](https://www.nerdfonts.com/) である必要があります。Nerd Font のアイコンは 1 桁として幅を計算します。これらの文字は East Asian Width が Ambiguous なので、端末で Ambiguous な文字を 2 桁で表示する設定にしていると、表示がずれたり行からはみ出したりします（クリックしたときにフォーカスするエージェントもずれます）。

Nerd Font のアイコンが 1 桁に収まるかどうかは、フォントと端末の組み合わせで変わります（Mono 版のフォントでもはみ出すことがあり、1 桁に収まるフォントでは端末によってアイコンが潰れて見分けられないことがあります）。名前までバッジに入れる（`badge_name` が既定で `true`、`app` のアイコンを名前の前に置く）のは、はみ出したアイコンがバッジの端で切れたり重なったりしないよう、アイコンの後ろに必ず文字が続くようにするためです。それでもアイコンの右が切れたり隣の文字と重なったりするとき（`badge_name "false"` のときや、幅が足りず状態アイコンだけになったときなど）は、`nerd_font_icon_padding "true"` を指定してください。

`show_cwd` で表示されるのは、ペインのルートプロセス（通常はシェル）のカレントディレクトリです。エージェントが中で別のディレクトリ（worktree など）に移っても追従せず、SSH 先やサンドボックス内で動くエージェントでは手元のシェルのディレクトリが表示されます。worktree ごとに見分けたいときは、そのディレクトリに `cd` してからエージェントを起動してください。

不正な値は既定値に戻し、エージェントがいないときにプラグインの行へ警告を出します。

## 開発

- ビルドターゲットは `.cargo/config.toml` で `wasm32-wasip1` に固定しています。ユニットテストはホストのターゲットを指定して実行します。

  ```sh
  cargo build
  cargo test --target x86_64-unknown-linux-gnu
  ```

- 動作確認は `zellij --layout dev.kdl` で行います。中央のペインで `scripts/osc7501-demo.sh` を実行すると、下から 2 行目に表示されます。再ビルドした後は、上の `bash` ペインで Enter を押すとタブごと開き直して読み込み直します。
- OSC 7501 のレコードは、送信元のプログラムが終わってシェルに戻ると約 2 秒で Zellij が消します。手動で送るときは送信元を終了させないでください（`scripts/osc7501.sh` は既定で Enter 待ちになります）。
- OSC 7501 のフォーマットと Zellij 側の挙動は `doc/osc7501-findings.md` にまとめています。
