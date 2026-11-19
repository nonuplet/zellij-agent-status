# 実装タスク

## 前提：Zellij 0.46 の OSC 7501 対応

Zellij 0.46 は OSC 7501 を自前でパースし、結果を `PaneUpdate` イベントの `PaneInfo.program_status` としてプラグインに渡す。
そのため、プラグイン側で生の OSC を解析する必要はない見込み。

- 定義：`zellij-utils-0.46.0/src/attention.rs`

```rust
pub struct ProgramStatusRecord {
    pub id: Option<String>,          // None = ルートレコード
    pub state: Option<ProgramState>, // idle / working / done / blocked / error
    pub kind: Option<BlockedKind>,   // permission / question / auth（blocked 時）
    pub progress: Option<u8>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub msg: Option<String>,
    pub updated_ms: u64,
}
```

- 送信フォーマットは、Zellij がホストへ送るときの形（`HostProgramStatus::to_osc`）から次のように推測している。受信側の仕様は未確認なので、T0 で確かめる。
  - `ESC ]7501;state=working:app=claude:msg=<base64> ESC \`
  - クリア：`ESC ]7501;state=clear ESC \`
- `PaneInfo.attention`（`PaneAttention`）には、Zellij が program status やベルなどから組み立てた状態も入っている。どちらを使うかは T0 で判断する。

## フェーズ0：検証（先行して1人で行う）

### T0. OSC 7501 の受信確認

- `PaneUpdate` を subscribe し、`program_status` と `attention` をログに出す。
- ペインから `printf` で OSC 7501 を送り、届くことを確認する。
- 確認すること：
  - 受け付けるキーと値の形式（`msg` の base64 など）
  - `id` 付きの複数レコードの扱い
  - `state=clear` の挙動
  - イベントが届く頻度
  - 必要な permission（`ReadApplicationState` の想定）
- 成果物：内部モデルの型定義（例：`AgentStatus { pane_id, tab, state, app, title, msg, progress, kind }`）。以降の並列作業はこの型を境界にする。

## フェーズ1：並列で進める作業（T0 の型が決まった後）

### A. 状態の収集

- `PaneManifest` / `TabUpdate` から `Vec<AgentStatus>` に変換する。
- permission の要求と、ペインが閉じたときの後始末。
- 変換は副作用のない関数にしてテストする。

### B. 表示

- `&[AgentStatus]` と幅から1行の文字列を作る。
- 状態ごとのアイコンと色（`ModeUpdate` のパレットを使う）、表示幅を考えた切り詰め、複数エージェントの並べ方。
- 副作用のない関数にしてテストする。

### C. 設定

- `load` の configuration を読む（表示フォーマット、`app` の絞り込み、記号かアルファベットかの切り替えなど）。
- B と項目が関係するので、設定のスキーマを先に決めてから並列で進める。

### D. 送信側とテスト用ツール

- OSC 7501 を送る手動テスト用スクリプト。
- Claude Code の hooks から送る例。hooks の stdout は端末につながっていないので、`/dev/tty` に書く必要がありそう。

## フェーズ2：統合

### E. 結合と確認

- `main.rs` で A・B・C をつなぐ。
- `dev.kdl` で動作を確認する。
- README には必要なことだけ追記する。

### F.（任意）クリックで該当ペインにフォーカス

## 注意点

- `.cargo/config.toml` のビルドターゲットが `wasm32-wasip1` 固定なので、ユニットテストはホスト側のターゲットを指定して実行する必要がある（例：`cargo test --target x86_64-unknown-linux-gnu`）。
