# zellij-agent-status

Zellij 用のプラグインを開発するRustプロジェクトのリポジトリ。
OSC 7501 を使い、 Zellij のターミナルで動いているエージェントのステータスを表示するプラグインを実装する。

## 依存 Crate の調査

- まずプロジェクト内の使用箇所から LSP（goToDefinition / hover）で定義へ移動し、Read で読む。
- 検索が必要なときは Bash の grep を使う。cd はせず、検索先を絶対パスで指定する。
  - 例: grep -rn "ProgramState" --include='*.rs' ~/.cargo/registry/src/index.crates.io-*/zellij-utils-0.46.0/
  - バージョンは Cargo.lock で確認し、該当 Crate のディレクトリに絞る。

## sandbox環境

echo ${SANDBOX_RUNTIME} をした時に `1` が帰ってきた場合、Claude Code付属のsandbox環境 (bubblewrap) で動作している。以下の内容に注意すること:

- `git` 操作のタイミングなど、ホスト側にない未定義ファイル (`.bashrc`, `.zshrc`, `.gitconfig`, `mcp.json` など) が見えることがある。もし見つけてもこれは無視して良い。報告も不要。
- `git push` はできないので、ユーザに任せる。
- `cargo` を使うタイミングで Read や　Write の問題にぶつかる可能性がある。もし遭遇したら無理に回避しようとせず、ユーザに判断を依頼する。
- cargo の registry 内を調べるときは、cd せずに絶対パスで指定する。または Grep / Read ツールを使う (~/.cargo/registry は作業ディレクトリなので、ブロックされない)

## ドキュメントに関するルール

- CLAUDE.md に新しい情報を追記する場合、プロジェクト固有の話題、分かりづらい内容のみ追加する。一目ですぐわかるような内容や、一般論で説明できる内容は追記しない。
- README.md に新しい情報を追記する場合、ユーザが開発時に知るべき内容に限り追加する。個々の実装についての詳細な解説は書かない。
  - 個々の実装の詳細は、実装されているファイルのコメントに書かれているべき。 README.md は網羅的なドキュメントではない。
  - 例外的に、一般的に違和感のある挙動・開発時にやってはいけない事などは記述する。