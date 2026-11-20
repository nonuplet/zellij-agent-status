# 実装タスク

## 前提：Zellij 0.46 の OSC 7501 対応

Zellij 0.46 は OSC 7501 を自前でパースし、結果を `PaneUpdate` イベントの `PaneInfo.program_status` としてプラグインに渡す。
プラグイン側で生の OSC を解析する必要はない。

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

- 送信フォーマット：`ESC ]7501;state=working:app=claude:msg=<base64> ESC \`
  - `state` は必須。`msg` と `title` は base64。送るたびにレコードが丸ごと置き換わる。
  - クリア：`ESC ]7501;state=clear ESC \`
- 方針：**ルートレコードの `program_status` を使う**。
  - `PaneInfo.attention` は done や error を表せないので使わない。
  - `id` 付きレコードは更新イベントが届かないことがあるので、当面は扱わない。
- 詳細は [osc7501-findings.md](osc7501-findings.md) を参照。

## フェーズ0：検証（先行して1人で行う）

### T0. OSC 7501 の受信確認

- 完了。検証結果は [osc7501-findings.md](osc7501-findings.md) にまとめてある。
- 成果物：内部モデルの型 `AgentStatus` / `AgentState`（`src/model.rs`）。以降の並列作業はこの型を境界にする。
- `ProgramStatusRecord` などを型名で参照できるよう、`zellij-utils` を直接の依存に加えた（バージョンは zellij-tile にそろえる）。

## フェーズ1：並列で進める作業（T0 の型が決まった後）

### A. 状態の収集

- `PaneUpdate`（`PaneManifest`）のルートレコードから `Vec<AgentStatus>` に変換する。タブ名が必要なら `TabUpdate` も使う。
- permission（`ReadApplicationState`）の要求。許可が得られるまでは selectable のままにする。
- レコードは送信元のプログラムが終わると Zellij が自動で消すので、プラグイン側で古いレコードを掃除する必要はない。
- 変換は副作用のない関数にしてテストする。

### B. 表示

- `&[AgentStatus]` と幅から1行の文字列を作る。
- 状態ごとのアイコンと色（`ModeUpdate` のパレットを使う）、表示幅を考えた切り詰め、複数エージェントの並べ方。
- 副作用のない関数にしてテストする。
- 実装済み（`src/view.rs`）。使う側が知っておくことは [view-notes.md](view-notes.md) にまとめてある。

### C. 設定

- `load` の configuration を読む（表示フォーマット、`app` の絞り込み、記号かアルファベットかの切り替えなど）。
- B と項目が関係するので、設定のスキーマを先に決めてから並列で進める。
- 実装済み（`src/config.rs`）。設定項目は冒頭のコメントを参照。
  - `Config::from_configuration` で設定と警告メッセージを作る。`Config.view` を `render_line` に渡し、`apps` での絞り込みは `Config::accepts_app` で行う。
  - 表示の並びは B で固定しているので、表示フォーマットは設定項目にしていない。

### D. 送信側とテスト用ツール

- OSC 7501 を送る手動テスト用スクリプト。
- Claude Code の hooks から送る例。hooks の stdout は端末につながっていないので、`/dev/tty` に書く（子プロセスから書いても受理されることは T0 で確認済み）。

## フェーズ2：統合

### E. 結合と確認

- `main.rs` で A・B・C をつなぐ。
- `dev.kdl` で動作を確認する。
- T0 の probe（`src/probe.rs`、`t0.kdl`、`scripts/t0-cases*.sh`）を削除する。削除するなら、[osc7501-findings.md](osc7501-findings.md) の「再現方法」も合わせて直す。
- README には必要なことだけ追記する。

### F.（任意）クリックで該当ペインにフォーカス

## 注意点

- `.cargo/config.toml` のビルドターゲットが `wasm32-wasip1` 固定なので、ユニットテストはホスト側のターゲットを指定して実行する必要がある（例：`cargo test --target x86_64-unknown-linux-gnu`）。
