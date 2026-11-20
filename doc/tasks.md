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
- 成果物
  - `scripts/osc7501.sh`：1 件送る。送信元が終わるとレコードが消えるので、既定では送信後に Enter 待ちでとどまる。
  - `scripts/osc7501-demo.sh`：全状態を順に送り続ける（B の表示確認用）。
- Claude Code の hooks から送る例（`examples/claude-code/`）も作ったが、E で削除した。
  - Claude Code の最新版は OSC 7501 を自分で送るので、hooks は不要。hooks なしでプラグインに表示されることを実機で確認した。
  - そもそも hooks からは `/dev/tty` に書けない見込み（[osc7501-findings.md](osc7501-findings.md) の「Claude Code の hooks から送る場合」）。
  - 中身はコミット `c6bf100` / `53d7179` に残っている。

## フェーズ2：統合

### E. 結合と確認

- `main.rs` で A・B・C をつなぐ。
- `dev.kdl` で動作を確認する。
- T0 の probe（`src/probe.rs`、`t0.kdl`、`scripts/t0-cases*.sh`）を削除する。削除するなら、[osc7501-findings.md](osc7501-findings.md) の「再現方法」も合わせて直す。
- README には必要なことだけ追記する。
- 実施済み
  - `main.rs` で A・B・C をつないだ。`ModeUpdate` で色を更新し、`apps` の絞り込みは表示の直前に行う。
  - permission の応答待ち・拒否時と、エージェントがいないときの設定の警告は、`view::render_message` で 1 行のメッセージとして出す。警告は Zellij のログにも出す。
  - probe 一式を削除し、[osc7501-findings.md](osc7501-findings.md) の「再現方法」を直した。
  - 不要になった hooks の例（`examples/claude-code/`）を削除した（D を参照）。
  - `dev.kdl` に、スクリプトを実行するためのシェルペインを足した。
- 完了。実機（`dev.kdl`）で次の項目を確認し、すべて問題なかった。
  - permission の許可と、許可後の表示。
  - `ModeUpdate`：`zellij --layout dev.kdl options --theme retro-wave` で起動し、アイコンの色がテーマの色になる。
  - Claude Code の状態ごとの表示
    - idle：起動直後。
    - working：時間のかかる指示を出している間。
    - done：応答が終わった後。
    - blocked / permission：許可ダイアログが出ている間。許可した後・拒否した後に blocked のまま残らない。
    - blocked / question：AskUserQuestion で質問が出ている間。
    - Esc で中断した後に working のまま残らない。
    - error：`claude --model` に存在しないモデル名を渡して指示を出したとき。
  - Claude Code を終了すると、約 2 秒で表示が消える。
  - 複数のペイン・複数のタブで Claude Code を動かしたときの並び順（タブ順 → ペイン id 順）と、端末の幅を狭めたときの切り詰め。
  - 設定：`icon_style "alphabet"`、`apps "claude"`（`scripts/osc7501-demo.sh` の `demo` が隠れる）、不正な `icon_style` の警告。

### F.（任意）クリックで該当ペインにフォーカス

## 注意点

- `.cargo/config.toml` のビルドターゲットが `wasm32-wasip1` 固定なので、ユニットテストはホスト側のターゲットを指定して実行する必要がある（例：`cargo test --target x86_64-unknown-linux-gnu`）。
