# zellij-agent-status

Zellij のステータスバーにAIエージェントのステータスを表示するためのプラグインです。

OSC 7501 を読んでステータスの情報を取得します。Zellij 0.46 以降が必要です。

## 使い方

- 設定項目は `src/config.rs` の冒頭のコメントを参照してください。
- Claude Code（最新版）は自分で OSC 7501 を送るので、hooks などの設定なしで表示されます。
- 手動で送るときは `scripts/osc7501.sh` を使います。
- 初回の起動時は、プラグインのペインに permission（`ReadApplicationState`）の確認が出ます。1 行のペインなので全文は読めませんが、ペインをクリックしてフォーカスし、許可してください。許可するまでエージェントの状態は表示されません。

## 開発

- ビルドターゲットは `.cargo/config.toml` で `wasm32-wasip1` に固定しています。ユニットテストはホストのターゲットを指定して実行します。

  ```sh
  cargo build
  cargo test --target x86_64-unknown-linux-gnu
  ```

- 動作確認は `zellij --layout dev.kdl` で行います。中央のペインで `scripts/osc7501-demo.sh` を実行すると、下から 2 行目に表示されます。再ビルドした後は、上の `bash` ペインで Enter を押すとタブごと開き直して読み込み直します。
- OSC 7501 のレコードは、送信元のプログラムが終わってシェルに戻ると約 2 秒で Zellij が消します。手動で送るときは送信元を終了させないでください（`scripts/osc7501.sh` は既定で Enter 待ちになります）。
