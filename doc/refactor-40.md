# リファクタリングの作業メモ（#40）

#40 の作業中だけ使う一時的なドキュメント。作業が終わったら削除する。
調査結果と、Issue で決めた方針をまとめている。チェックリストは Issue #40 の本文を正とする。

振る舞いを変えないリファクタリングに限る。次のものは維持する。

- `model` を境界に、収集と表示を分けていること
- view が副作用のない関数だけでできていること
- Collector が順序を持たないこと（#13）

## 進め方

依存関係から、次の順に進める。コミットは段階ごとに分ける（必要なら PR も分ける）。

1. view のモジュール分割（1-1、1-2）
2. 設定の読み込みの共通化（2-1、2-3）
3. 幅の割り当てとバッジの整理（3-1〜3-4、2-2、2-4）
4. Collector の整理（4-1〜4-3）
5. 細かい点（5）

## 完了条件

- 表示の結果（`render_line` / `render_message` の出力）が変わらない
- 既存のユニットテストが通る（`cargo test --target x86_64-unknown-linux-gnu`）。テストをまとめたり移したりしても、確かめている内容は減らさない
- `cargo clippy` で警告がない
- `zellij --layout dev.kdl` で実機の表示が変わらないことを確認する

---

## 1. 1 ファイルの責任範囲

### 1-1. `src/view/layout.rs` に 7 つの責務が入っている

**今の実装**

約 2060 行のうち、ロジックが約 630 行、テストが約 1430 行（`mod tests` 以降）。ロジックには次のものが同居している。

| 責務 | 主なシンボル |
|---|---|
| 描画の中間表現 | `Role` / `Fill` / `Span`（`paint.rs` もここから import している） |
| バッジの形 | `BadgeShape`（config も使う公開型）/ `BadgeEnds` |
| 1 エージェント分の内容づくり（名前の決定を含む） | `Item::new` |
| 1 エージェント分の Span 組み立て | `Item::spans` |
| 幅の割り当て | `Item::widths` / `plan` / `shown` / `assemble` / `fit` / `fair_share` |
| 文字列ユーティリティ | `clean` / `dir_name` / `take_width` / `truncate` / `clamp` |
| 絞り込み・並び順・全体の組み立て | `layout` |

**問題**

- バッジ・幅の計算・名前の決め方のどれを変えるときも、このファイルを触ることになる。
- 冒頭のモジュールコメント（40 行）が、すべての組み合わせの説明を 1 か所に抱えている。
- `paint.rs` と `mod.rs` は「layout の中の型」に依存している。

**方針**

```
src/view/
  mod.rs        render_line / render_message（入口だけ）
  options.rs    ViewOptions, MsgMode, DEFAULT_MAX_LENGTH
  span.rs       Span / Role / Fill
  text.rs       clean / truncate / take_width / clamp / ELLIPSIS / dir_name
  badge.rs      BadgeShape / BadgeEnds（＋ バッジの範囲と、設定の解決。3-1・2-2 を参照）
  item.rs       Item（内容の決定と Span の組み立て）
  fit.rs        Widths / plan / fit / fair_share（幅の割り当て）
  layout.rs     layout()：絞り込み・並び順・組み立て
```

- テストはそれぞれのモジュールに移す。行全体のテスト（`plain_with` を使うもの）は、`layout/tests.rs` のように別ファイルに分ける。
- モジュールコメントも、該当するファイルに分けて移す（例：縮める順は `fit.rs`、バッジにする範囲は `badge.rs`）。
- 分割の段階では純粋な移動だけにし、ロジックは変えない（3 で行う）。

### 1-2. `src/view/mod.rs` が入口・設定・絞り込み・テスト用ヘルパーを兼ねている

**今の実装**

次のものが 1 ファイルにある。

- `ViewOptions`（13 フィールド）と `MsgMode`
- 絞り込み（`shows` / `accepts_app`）
- 描画の入口
- 他のモジュールが使うテスト用ヘルパー（`pub(crate) mod tests` の `agent` / `with_msg` / `with_cwd`）

**方針**

- `ViewOptions` / `MsgMode` / `DEFAULT_MAX_LENGTH` は 1-1 の `options.rs` に移す（今は `mod.rs`・`layout.rs`・`icon.rs` に散らばっている）。
- テスト用ヘルパーは、テスト専用のモジュール（例：`view/test_support.rs`、`#[cfg(test)]`）に分ける。
- `accepts_app` は `shows` とテストからしか使われていないので、`pub` をやめる。

---

## 2. オプションを追加しやすい構造か

### 2-1. `Config::from_configuration` が 12 個のほぼ同じブロックの繰り返し

**今の実装**

`from_configuration` は、キーごとに `get → parse_* → match → 警告文を format!` を書いている。

**問題**

- 真偽値のオプションを 1 つ足すと、次のものがすべて必要になる。
  - ブロック 1 つ
  - テスト 2 つ（`xxx()` と `invalid_xxx_falls_back...`。中身はほぼ同じ）
  - `config.rs` の表の 1 行
  - README の表の 1 行
  - `ViewOptions` のフィールドと `Default`
- 警告文の「`symbol`・`alphabet`・… のどれか」と `parse_icon_style` の `match` が別々に書かれている。片方だけ直すと、表示と実際に受け付ける値がずれる。

**方針**

読み込みを小さなヘルパーにまとめ、列挙型は「文字列 → 値」の表を 1 つだけ持つ。

```rust
struct Reader<'a> {
    configuration: &'a BTreeMap<String, String>,
    warnings: Vec<String>,
}

impl Reader<'_> {
    /// 値があれば `parse` して `target` に入れる。不正なら警告を積み、`target` は既定値のまま
    fn read<T>(&mut self, key: &str, target: &mut T, parse: impl Fn(&str) -> Result<T, String>) {
        let Some(value) = self.configuration.get(key) else { return };
        match parse(value) {
            Ok(v) => *target = v,
            Err(expected) => self.warnings.push(format!(
                "{key}: `{value}` は使えません。{expected}を指定してください（既定値を使います）"
            )),
        }
    }
}

const ICON_STYLES: &[(&str, IconStyle)] = &[("symbol", IconStyle::Symbol), /* ... */];
// choice(ICON_STYLES) が parse と「`symbol`・`alphabet`・… のどれか」の両方を表から作る
```

本体は 1 キー 1 行になる。

```rust
r.read("show_cwd", &mut view.show_cwd, parse_bool);
r.read("icon_style", &mut view.icons, choice(ICON_STYLES));
```

- `nerd_font` を `icon_style` より先に読む順序の依存は残る。コメントで理由を残す。
- `parse_max_length` の戻り値（`Option<Option<usize>>`）は `Result<Option<usize>, String>` にする（5 も参照）。`max_length` の警告文は既定値を含むなど形が違うので、`Err` の文言で表す。
- テストは、真偽値のキーを `(キー, フィールドを読む関数)` の表にして、テーブル駆動の 1 本にまとめる。

### 2-2. 互いに依存する設定の組み合わせが、コメントでしか表されていない

**今の実装**

`ViewOptions` は 13 個のフィールドを平らに並べた構造体で、次の依存はコメントでしか表されていない。

- `badge_shape` は `nerd_font` が false なら使わない
- `badge_name` は絵文字アイコンでは効かない
- `nerd_font` は `icons` の既定値を変える（config で解決している）
- `nerd_font_icon_padding` は Nerd Font のアイコン（`icon_style nerd` か `nerd_font` の `app` のアイコン）があるときだけ効く

今は `BadgeEnds::new` が一部を解決しているが、残りは `Item` の bool のフィールド（`highlight` / `badge_name` / `nerd_icon` / `icon_padding`）にばらばらに写している。

**決定した方針**

- `ViewOptions` は設定の写しとして平らなままにし、入れ子の構造体には分けない。
  - 問題は設定同士の依存がコメントでしか表されていないことで、入れ子にしても解決しない。
  - 入れ子にすると、テストの `ViewOptions { .., ..base() }` の書き方が長くなる（数十か所）。
  - `nerd_font_icon_padding` のように、どのグループにも収まらない設定がある。
- 代わりに、互いに依存する設定（`icons` / `nerd_font` / `badge_shape` / `badge_name` / `highlight_attention` / `nerd_font_icon_padding`）の組み合わせの解決を、view の中の 1 か所（例：`BadgeStyle::new(&options)`）にまとめる。
  - 端の文字、バッジにする範囲（3-1 の `BadgeExtent`）、アイコンの後ろに空白を置くかを、ここで一度に決める。
  - `Item` や Span の組み立ては、解決済みの型だけを見る。
  - 3-1 と一緒に行う。

### 2-3. オプションの説明が 3 か所に重複している

**今の実装**

次の 3 か所に、同じ内容がそれぞれ少しずつ違う言い回しで書かれている。

- README の表
- `config.rs` 冒頭の表
- `ViewOptions` の各フィールドのドキュメント

**方針**

- ユーザ向けの一覧は README だけにする。
- `config.rs` には分かりにくい規則だけを残す（`nerd_font` を先に読む理由、未知のキーを無視する理由、真偽値の受け付け方など）。
- フィールドのドキュメントは、実装上の意味に絞る。

### 2-4. #7（クリックでフォーカス）に向けた下準備

**今の実装**

`render_line` は `String` しか返さない。`Span` には、どのエージェントの部分かという情報がない。
`clamp` は切り詰めるときに `Span::new(text, span.role).with_fill(span.fill)` で作り直しているので、Span にフィールドを足すとここで抜け落ちる。

**決定した方針**

- #7 では、`Span` にペイン id（`Option<u32>`）を持たせ、描画した Span 列の幅から範囲を求める。
  - 実際に描画した結果から求めるので、表示とずれない。行頭の余白や区切りは `None` にすれば、クリックしても何も起きない場所として扱える。
  - `layout` が範囲の一覧（`Vec<(Range<usize>, u32)>`）を別に返す方式は採らない。行末を切る `clamp` の後で一覧も切り直す必要があり、同じ情報を 2 か所で計算することになる（3-4 と同じ問題）。
- フィールドの追加は #7 で行う。今回足しても読む側がなく（`dead_code` の警告が出る）、振る舞いを変えない範囲からも外れるため。
- 今回は下準備だけを行う。
  - `clamp` が切り詰めるときに Span を作り直さず、他のフィールドを引き継ぐようにする（`Span { text, ..span }`）
  - Span を作る場所を、`Item` の組み立てと行頭・区切り・`clamp` に限る形を保つ（1-1 と 3-4 で自然にそうなる）
- 次のものは #7 で行う。
  - `render_line` の戻り値の変更
  - `State` が最後に描画した Span 列を持つこと
  - テスト用ヘルパー `agent_widths` が区切りを文字列（`SEPARATOR`）で見分けている部分の修正

---

## 3. 必要以上に複雑なロジック

### 3-1. `Item::spans` のバッジの閉じ方が、副作用に頼っている

**今の実装**

```rust
let parts = [
    (head,  self.ends.is_some() || self.highlight),
    (label, self.badge_name || self.highlight),
    (msg,   self.highlight),
];
for (part, in_badge) in parts {
    if in_badge { /* 塗る */ } else {
        spans.append(&mut right);  // append で right が空になるので 1 回しか閉じない
        ...
```

**問題**

- 「塗る部分は必ず先頭から連続している」という前提が、3 つの bool の組み合わせでしか保証されていない。
- `append` で空になることを利用して、右端を 1 回だけ付けている。
- 前提が崩れる変更を入れると（例：msg だけ塗るオプション）、表示が静かに壊れる。

**方針**

「先頭から何部分を塗るか」を列挙型にして、前提を型で表す。

```rust
/// バッジに入れる範囲。先頭から連続する
enum BadgeExtent { None, Head, Label, Whole }
```

- 2-2 の `BadgeStyle::new` で一度だけ計算し、`spans` では先頭から `extent.parts()` 個の部分の直後に右端を差し込む。
  - メソッド名は `len` ではなく `parts` にした。`len` だと clippy の `len_without_is_empty` の対象になる名前で、部分の数を表していることも伝わりにくいため。
- 絵文字アイコンでの強調（端の文字なしで全体を塗る）は、`ends: None` と `extent: Whole` の組み合わせで表す。
- `Item` の bool のフィールドを減らす。

### 3-2. 幅を割り当てる `plan` → `shown` → `assemble` が、詰めたベクタの対応付けに頼っている

**今の実装**

- `shown` で「その部分を表示するエージェントの分だけ」に詰めたベクタを作る。
- `fit` で分けた後、`assemble` が `caps` の `Some` の位置にイテレータで順に戻す。

**問題**

- 添字の対応が暗黙。
- `Widths.name` は「`Item::widths` では常に `Some`、`assemble` の最後の段階でだけ `None`」という、コメントでしか守られていない不変条件を持っている。

**方針**

詰めずに `&[Option<usize>]`（エージェント数と同じ長さ）のまま `fit` / `fair_share` に渡し、`Some` の要素だけで分ける。`shown` と `assemble` はなくなる。

```rust
if let Some(msgs) = fit(&caps.msg, room, MIN_MSG_WIDTH) {
    return zip(names, cwds, msgs);
}
```

あわせて、`plan` が行頭の余白の幅も引数で受け取るようにする。今は区切りの幅は引数（`sep_width`）で受け取る一方、行頭の余白は `layout::PAD` を直接読んでいて、`fit` と `layout` が互いに依存している（1 の分割で見つかった）。

### 3-3. msg → cwd → 名前の順に縮める処理が、2 か所に別々に書かれている

**今の実装**

- `Item::widths`：1 エージェント分を `max_length` に収める
- `plan`：行の幅に収める

どちらも「msg を切り詰め → 省いて cwd → 省いて名前」の段階を、それぞれ別のコードで書いている。

**方針**

要素が 1 つなら、`fair_share` は `min(width, room)` になり、`fit` の判定は `Item::widths` の `fits` と同じになる。そのため、段階的に縮める関数を 1 つにして、「1 要素・`max_length`」と「全要素・`cols`」の 2 回呼ぶ。

- 行の幅の計算には行頭の余白と区切りが入り、1 エージェント分には入らない。この差は呼び出し側で `room` に反映する。
- `max_length` に収める処理（今の `Item::widths`）は `fit` に移す。
  - 中身は幅の割り当てなのに `item.rs` にあるため、`fit` は `Item` と `CWD_OVERHEAD` を、`item` は `Widths` と `MIN_MSG_WIDTH` / `MIN_CWD_WIDTH` を使い、`item` と `fit` が互いに依存している（1 の分割で見つかった）。
  - 1 では純粋な移動に徹するため、`item.rs` に残している。
  - 移した後も、`Item::spans` が `Widths` を受け取るので、`item` は `fit` の `Widths` を使う。循環を完全になくすなら、`Widths` を `item` 側に置く（`Item` の各部分の幅なので、置き場所としても自然）。そうすると `fit` が `item` を使うだけの一方向になる。
- 既存の `never_exceeds_width` / `never_exceeds_max_length` のテストで、表示が変わらないことを確かめる。

### 3-4. 幅の計算と Span の組み立てが、別々に実装されている

**今の実装**

`head_width` / `name_overhead` / `msg_separator().width()` / `CWD_OVERHEAD = 3` で幅を数えている。実際の文字列（`" ("` / `")"`、アイコンの後ろの空白など）は、`Item::spans` で別に作っている。

**問題**

`spans` に空白を 1 つ足して幅の計算を直し忘れると、行からはみ出す。いまは組み合わせを総当たりするテスト（`option_combinations`）でだけ守られている。

**方針**

固定部分（アイコン・進捗・端・区切り）は先に Span として作り、その幅を Span から測る。幅の情報を 1 か所にまとめる。3-1 と合わせて行う。

---

## 4. データ収集と表示の分離

### 4-1. Collector が permission とペインの selectable も扱っている

**今の実装**

`Collector::load` / `update` は、`request_permission`、`set_selectable(false)`、`Permission` 型の定義も持っている。

**問題**

これらはデータ収集ではなく、プラグインのペインのライフサイクル。#7 で `ChangeApplicationState` を足すと、Collector が「収集に要らない permission」まで要求することになる。

**方針**

permission の状態とペインの操作は `State`（または `src/permission.rs`）に移す。Collector は、許可の後に届くイベントの変換だけにする。

### 4-2. cwd・タブ名が、側の表と `AgentStatus` の 2 か所に保存されている

**今の実装**

- `cwds` / `tab_names` を持ちつつ、各 `AgentStatus` にも `cwd` / `tab_name` をコピーしている。
- `set_cwd` は両方を更新している。
- `TabUpdate` では、一覧全体を clone してから付け直している。
- 再描画するかどうかは、結合した後の `AgentStatus` の一覧同士を比べて決めている（`Collector::replace`）。

**問題**

同期が必要な状態が 2 つある。`PaneUpdate` の分岐は、この同期のために読みにくくなっている。

**決定した方針**

Collector は次の 3 つの表だけを持ち、`statuses()` で結合して `AgentStatus` を作る。

| 表 | 元のイベント | 中身 |
|---|---|---|
| エージェントの表 | `PaneUpdate` | ペイン id → (タブの位置, OSC 7501 のレコードから作った状態) |
| タブ名の表 | `TabUpdate` | タブの位置 → タブ名 |
| cwd の表 | `get_pane_cwd` / `CwdChanged` | ペイン id → cwd |

再描画するかどうかは、各イベントが更新した表が変わったかで判定し、結合した結果同士は比べない。

- エージェントのいないタブの名前の変更や、タブの追加・削除・移動でも再描画するようになる。どれもまれな操作で、再描画も 1 行描き直すだけなので許容する。
- フォーカスの移動などで届く `PaneUpdate` / `TabUpdate` は、表が変わらないので今までどおり再描画しない。
- エージェントの表には OSC のレコードを持つペインだけを入れる。
- cwd の扱い（エージェントでなくなったペインの cwd は捨てる、取得に失敗したペインは取得し直さない、など）は今の振る舞いを保つ。`collect.rs` のモジュールコメントにある規則を確認しながら移す。

### 4-3. 表示に使わない `kind` / `updated_ms` が、再描画の判定に影響している

**今の実装**

`AgentStatus.kind` と `updated_ms` は、収集側とテスト以外では読まれていない（LSP の参照で確認）。

**問題**

- `updated_ms` は同じ状態の再送でも変わる。そのため、見た目が変わらなくても再描画される。
- `kind: Option<BlockedKind>` は zellij の型をそのまま持っている。これは `AgentState` を独自に定義した理由（「zellij-utils から切り離す」）と食い違う。

**決定した方針**

エージェントの表（4-2）には、OSC 7501 のレコードをそのまま入れず、表示に使うフィールドだけに変換して入れる。`updated_ms` と `kind` は、使うまで持たない。

---

## 5. Rust らしくない、細かい点

| 箇所 | 今の書き方 | 方針 |
|---|---|---|
| `layout.rs` の `clean(text: Option<&str>)` | `Option` を引数に取っている。`clean(Some(&dir_name(cwd)))` のような呼び方になる | `clean(&str) -> Option<String>` にし、呼び出し側で `.and_then(clean)` にする |
| `config.rs` の `parse_max_length -> Option<Option<usize>>` | 二重の `Option` で「不正」と「上限なし」を区別している | `Result<Option<usize>, String>`（2-1 の `Reader` と揃う） |
| `paint.rs` の `out.push_str(&format!(..))` と、`String` を返す `Color::sgr` | Span ごとに文字列を確保している | `use std::fmt::Write` で `write!(out, ..)` にし、SGR は `Display` を実装した型にする |
| `collect.rs` の `pub fn collect` | モジュールの外では使われていない | `pub` をやめる |
| `main.rs` の `MESSAGE_PREFIX` と `collect.rs` の `eprintln!` | 接頭辞 `zellij-agent-status: ` が 2 か所にある | 定数か、小さなログ用の関数にまとめる |
| `layout.rs` の `Item::new` の `.filter(\|_\| options.show_tab_name)` | `Option::filter` で外のフラグを見ている | 動作は正しい。名前の決定を関数に分けるときに、`if` で書いたほうが読みやすいかを検討する |
| テスト用ヘルパー `agent_widths` | 区切りを `span.text == SEPARATOR` で見分けている | #7 で直す（2-4） |
