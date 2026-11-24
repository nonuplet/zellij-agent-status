# OSC 7501 と Zellij 0.46 の挙動

Zellij 0.46 で、ペインから送った OSC 7501 がプラグインからどう見えるかを確かめた結果。
検証は 2026-11 に実施した。値を 1 件ずつ送って確かめるには `scripts/osc7501.sh` を使う。

Zellij 0.46 は OSC 7501 を自前でパースし、結果を `PaneUpdate` の `PaneInfo.program_status` としてプラグインに渡す。プラグイン側で生の OSC を解析する必要はない。
型の定義は `zellij-utils-0.46.0/src/attention.rs` にある。

## このプラグインの方針

- **ルートレコード（`id` が None のもの）の `program_status` だけを使う。**
  - `PaneInfo.attention` は done や error を表せないので使わない。
  - `id` 付きレコードは、更新しても `PaneUpdate` が届かないことがあるので扱わない。
- 理由はそれぞれ下の「プラグインから見えるデータ」「イベントの届き方」を参照。

## プラグインから見えるデータ

- 受信した内容は `PaneInfo.program_status: Vec<ProgramStatusRecord>` に入る。
  - `id` なしのルートレコードと、`id` 付きのレコードが同じ Vec に並ぶ。
  - `msg` と `title` は base64 デコード済みの文字列で入る。
- `PaneInfo.attention` は、Zellij が program status などから組み立てた状態。
  - working と blocked は反映される。blocked は `NeedsYou` になり、text は `"needs approval: <msg>"` や `"has a question"` になる。
  - **done と error は反映されず、Idle になる。** フォーカスの有無に関係ない。
  - 複数レコードがあると、優先度の最も高い状態が使われる（例：ルートが working、`id=b` が blocked なら NeedsYou）。
- `ProgramStatusRecord` と `ProgramState` は zellij-tile の prelude に含まれていない。型名を書くには `zellij-utils` に直接依存する必要がある（このプラグインは依存している）。

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

- 状態の収集に使うイベントを受け取るのに必要な permission は `ReadApplicationState`。このプラグインは、ほかの操作のための permission も要求する（一覧は `src/permission.rs`）。
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

- 送信元のプログラムが終わってシェルに戻ると、約 2 秒以内に Zellij がそのペインの全レコードを消す。そのため、プラグイン側で古いレコードを掃除する必要はない。
  - プロセスの終了がきっかけなのか、プロンプトの表示がきっかけなのかは切り分けていない。
- 子プロセスから送信して、その子プロセスだけが終わった場合（親は動き続けている場合）は消えない。
- stdout と stderr を捨てた子プロセスが `/dev/tty` に書いた場合も、受理されて保持される。

## プラグイン実装上の注意

- permission ダイアログはプラグインのペインに出るので、`set_selectable(false)` にしているとユーザが応答できない。このプラグインは、許可が得られるまで selectable のままにしている（`src/permission.rs`）。
  - unselectable の状態で本当に応答できないかは試していない。
- 要求する permission を増やすと、以前に許可していても起動時にダイアログがまた出る。増やした分だけでなく、要求したすべての permission の許可を求められる。
- `set_selectable(false)` にしたペインにも、マウスのクリック（`Event::Mouse`）は届く。
- `focus_terminal_pane` で別のタブにあるペインを指定すると、そのタブにも切り替わる。`switch_tab_to` を呼ぶ必要はない。
