# zellij-agent-status

Zellij のステータスバーにAIエージェントのステータスを表示するためのプラグインです。

OSC 7501 を読んでステータスの情報を取得します。Zellij 0.46 以降が必要です。

## 使い方

- Claude Code（最新版）は自分で OSC 7501 を送るので、hooks などの設定なしで表示されます。エージェントごとの挙動は `doc/agent-behaviors.md` を参照してください。
- 手動で送るときは `scripts/osc7501.sh` を使います。
- 初回の起動時は、プラグインのペインに permission（`ReadApplicationState`）の確認が出ます。1 行のペインなので全文は読めませんが、ペインをクリックしてフォーカスし、許可してください。許可するまでエージェントの状態は表示されません。

## 設定

レイアウトの plugin ブロックに書きます。

```kdl
pane size=1 borderless=true {
    plugin location="file:/path/to/zellij-agent-status.wasm" {
        icon_style "alphabet"
        apps "claude-code"
        show_app "false"
        blocked_first "true"
        max_length 40
    }
}
```

| キー | 既定値 | 内容 |
|---|---|---|
| `icon_style` | `symbol` | アイコンを記号（`symbol`）、アルファベット 1 文字（`alphabet`）、絵文字（`emoji`）のどれにするか。絵文字は状態の色で塗られない |
| `apps` | （空） | 表示する `app` の一覧。カンマか空白で区切る。空なら絞り込まない。指定したときは `app` のないレコードを表示しない |
| `show_app` | `true` | `app` を名前の候補にするか（`true` / `false`）。名前は `title` → タブ名 → `app` → `#<pane_id>` の順で決まり、`false` なら `app` を飛ばす。`apps` の絞り込みには影響しない |
| `blocked_first` | `false` | 操作待ち（`Blocked`）のエージェントを先頭に並べるか（`true` / `false`）。`false` なら常にタブ・ペインの順。`Blocked` 同士はタブ・ペインの順で、`Blocked` でなくなると元の位置に戻る |
| `max_length` | `30` | 1 エージェント分（アイコン・進捗を含む）の表示幅の上限。12 以上の整数。超える分は msg から切り詰める。`0` なら上限なし |

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
