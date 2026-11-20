# 表示（タスク B）の引き継ぎメモ

`src/view.rs` を使う側（C・E）が知っておくべきことをまとめる。
表示の仕様の細部は `src/view.rs` のコメントとテストを参照。

## 公開しているもの

| 名前 | 用途 |
|---|---|
| `render_line(&[AgentStatus], cols, &Theme, &ViewOptions) -> String` | 1 行分の文字列（ANSI エスケープ付き）を返す。行末は背景色の空白で `cols` まで埋める |
| `Theme::from_styling(&Styling)` | `ModeInfo.style.colors` から色を作る |
| `Theme::default()` | `ModeUpdate` が届くまでの色 |
| `ViewOptions { icons: IconStyle }` | 表示の設定。`IconStyle::Symbol`（既定）/ `IconStyle::Alphabet` |
| `render_message(text, cols, &Theme) -> String` | エージェント一覧の代わりに出す 1 行のメッセージ（permission の案内や設定の警告）。制御文字の置き換えと切り詰めを行う |

## C（設定）向け

- 設定から作るのは `ViewOptions` だけ。項目を増やすときはこの構造体に足す。
- 今ある項目は、アイコンを記号にするかアルファベットにするかの切り替えだけ。

## E（結合）向け

- `ModeUpdate` を購読し、受け取るたびに `Theme::from_styling(&mode_info.style.colors)` で色を作り直す。
- `render` では `print!("{}", render_line(...))` を呼ぶだけでよい。並べ替え・切り詰め・色付けは `render_line` の中で行う。

## 共通の注意

- アイコンや切り詰め記号に、East Asian Width が Ambiguous の文字（`●`、`…` など）は使わない。日本語環境の端末で 2 桁になり、行がずれる。切り詰め記号が `…` ではなく `~` なのはこのため。
- `msg` / `title` は外部のプログラムから届く文字列なので、制御文字は表示前に空白へ置き換えている。表示に別の経路を作るときも同じ処理をすること。
- 並び順は (タブ, ペイン) で固定している。状態の優先度で並べ替えると、状態が変わるたびに位置が入れ替わるため。
- タブ番号やタブ名は表示していない。
