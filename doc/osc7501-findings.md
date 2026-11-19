# OSC 7501 の検証結果（T0）

Zellij 0.46 で、ペインから送った OSC 7501 がプラグインからどう見えるかを確かめた結果。
検証は 2026-11 に実施した。

## 再現方法

- `src/probe.rs`：検証用の probe。受け取ったイベントを画面に表示し、`target/t0-probe.log` にも追記する。
- `t0.kdl`：検証用のレイアウト。`zellij --layout t0.kdl` で起動する。
- `scripts/t0-cases.sh` / `scripts/t0-cases2.sh`：テストケースを 1 件ずつ送るスクリプト。左下のペインで実行する。

## プラグインから見えるデータ

- 受信した内容は `PaneInfo.program_status: Vec<ProgramStatusRecord>` に入る。
  - `id` なしのルートレコードと、`id` 付きのレコードが同じ Vec に並ぶ。
  - `msg` と `title` は base64 デコード済みの文字列で入る。
- `PaneInfo.attention` は、Zellij が program status などから組み立てた状態。
  - working と blocked は反映される。blocked は `NeedsYou` になり、text は `"needs approval: <msg>"` や `"has a question"` になる。
  - **done と error は反映されず、Idle になる。** フォーカスの有無に関係ない。
  - 複数レコードがあると、優先度の最も高い状態が使われる（例：ルートが working、`id=b` が blocked なら NeedsYou）。
- `ProgramStatusRecord` と `ProgramState` は zellij-tile の prelude に含まれていないので、`zellij-utils` に依存しない限り、プラグインから型名を書けない。フィールドの参照とメソッド（`as_str()` など）は使える。

## 送信フォーマット

`ESC ]7501;key=value:key=value... ESC \`（BEL 終端 `\a` も受理される）

| キー | 値 | 備考 |
|---|---|---|
| `state` | `idle` / `working` / `done` / `blocked` / `error` / `clear` | **必須**。`state` がないとシーケンスごと破棄される |
| `kind` | `permission` / `question` / `auth` | blocked のときに使う |
| `progress` | 0〜100 | 範囲外のときは `progress` だけが None になり、他のキーは受理される |
| `app` | 文字列（平文） | |
| `title` | base64 | 平文だとシーケンスごと破棄される |
| `msg` | base64 | 平文だとシーケンスごと破棄される。UTF-8（日本語）も可 |
| `id` | 文字列 | 付けると別レコードになる |

- 不正な値が 1 つでもあると、**シーケンス全体が捨てられる**。未知の `state`、平文の `msg` や `title` などが該当する。例外は `progress` の範囲外だけ。
- 送るたびに**レコードは丸ごと置き換わる**。前回の値とはマージされないので、毎回すべてのフィールドを送る。
- `id=<x>:state=clear` は、そのレコードだけを消す。
- ルートで `state=clear` を送ると、`id` 付きを含む**全レコードが消える**。
- query（`ESC ]7501;? ESC \`）への応答はない。

## イベントの届き方

- 必要な permission は `ReadApplicationState` だけ。
- ルートレコードを更新すると、内容が同じでも必ず `PaneUpdate` が届く。
  - attention が変わった場合の順序：`PaneAttentionChanged` → `PaneUpdate`（約 10ms 後）→ `TabUpdate` → `GlobalAttentionUpdate`
- **`id` 付きレコードだけの更新では、イベントが届かない。** ただし attention が変わる場合は届く。
  - このときも `get_pane_info()` でポーリングすれば、最新の内容を取得できる。
- 何も送っていないときは、イベントは流れない。
- フォーカスの移動など、program status 以外の変化でも `PaneUpdate` は届く。
- 頻度
  - 間隔なしで 20 回送ると、届くのは 3 回程度に間引かれる。最後の値は届く。
  - 100ms 間隔なら 20 回とも届く。

## レコードが消えるタイミング

- 送信元のプログラムが終わってシェルに戻ると、約 2 秒以内に Zellij がそのペインの全レコードを消す。
  - プロセスの終了がきっかけなのか、プロンプトの表示がきっかけなのかは切り分けていない。
- 子プロセスから送信して、その子プロセスだけが終わった場合（親は動き続けている場合）は消えない。
- stdout と stderr を捨てた子プロセスが `/dev/tty` に書いた場合も、受理されて保持される（Claude Code の hooks から送る想定）。

## プラグイン実装上の注意

- permission ダイアログはプラグインのペインに出るので、`set_selectable(false)` にしているとユーザが応答できない。許可が得られてから unselectable にする。
  - 今回は、probe を selectable のままにして回避した。unselectable の状態で実際に応答できないかは試していない。
