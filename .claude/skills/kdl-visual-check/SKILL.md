---
name: kdl-visual-check
description: 表示系の変更を人間が目視で検証するための、検証タスクの一覧と Zellij レイアウト（kdl）一式を tmp/ に作成する。「検証用の kdl を作って」「表示を目で確認したい」「検証タスクをリストアップして」「動作確認の準備をして」などの依頼で、必ず使う。
---

# 目視検証用の kdl の作成

エージェントが作るのは、検証タスクの一覧（会話に出す）と kdl ファイル（`tmp/` に置く）まで。
起動して結果を判断するのは人間で、エージェントの仕事は起動コマンドと期待する表示を渡したところで終わる。

## やらないこと

- **kdl を起動・検証しない。** `zellij` コマンド（`setup --check` などを含む）、パーサや lint、スクリプトの試し実行など、どの手段でも行わない。sandbox では Zellij の表示を確かめられず、確認は人間が起動した時点で済む。
- **kdl の書き方を調べない。** 必要な書き方は `dev.kdl` とこのファイルにすべてある。依存 Crate・Web・Zellij の設定・ほかの kdl を読みに行かない。ここにない書き方が要るときは、調べずに、ここにある書き方と人間の手順（操作やウィンドウ幅の変更など）で代える。それでも書けなければ、検証できない項目として報告する。
- **`tmp/` の外を変更しない。** `dev.kdl` と `scripts/` は変更しない。検証の準備中に実装の問題を見つけたら、直さずに報告する。

ユーザの指示がない限り、原則として、読むのは `dev.kdl`、今回の変更の内容（この会話か `git diff`）、期待する表示を決めるのに必要な `README.md`・`src/` だけにする。設定のキーと値、表示の形は `README.md` の「設定」にある。

## 手順

1. 変更の内容から、目で見ないと分からない項目を検証タスクとして洗い出す。ユニットテストで確かめられる文字列の組み立てなどより、色、記号の見え方、幅、クリックやキー操作、タブをまたぐ動き、既存の表示が崩れていないことを優先する。
2. タスクを検証シーンにまとめる。シーンは、同じ起動状態で続けて確認できるタスクの集まり。
3. `dev.kdl` を Read し、それをベースにして、シーンごとに 1 ファイル `tmp/<トピック>/NN-<シーン>.kdl` を Write で作る（例：`tmp/badge-shape/01-states.kdl`）。`tmp/` は作業中の worktree の直下のもの。同じ名前の別の検証があれば、上書きせずトピック名を変える。
4. 最後の「報告の形」で会話に出して終わる。チェックリストのファイルは作らない。

1 つのファイルに別のシーンのタブを混ぜない。プラグインはセッション内の全タブのエージェントを表示するので、シーン同士のエージェントが混ざる。タブを増やすのは、タブをまたぐ動きやタブ名を検証するときだけ。

## dev.kdl からの変更点

プロジェクトルートで起動する前提。`file:` とスクリプトの相対パスは、kdl の場所ではなく、起動したときのカレントディレクトリが基準になるので、`tmp/` に置いても書き換えない。

- 先頭のコメントを、そのシーンで確かめることに替える。
- 開き直し用の `bash` ペインの `--layout dev.kdl` を、そのファイルのパスに替える。
- 中央のペインの上に、エージェントのペインを足す。中央のペインは、手で操作するためのシェルとして残し、`focus=true` を付ける。
- プラグインの行に、検証する設定を書く。

### エージェントのペイン

1 件につき 1 ペイン。起動と同時にレコードを送り、Enter 待ちで残る。

```kdl
    pane split_direction="vertical" {
        pane command="bash" name="one" {
            args "-c" "scripts/osc7501.sh working -t one -p 40"
        }
        pane command="bash" name="two" {
            args "-c" "scripts/osc7501.sh blocked -k permission -t two -m 'Bash: rm -rf target'"
        }
    }
```

`args "-c" "<コマンド>"` の文字列に、送る内容をまとめて書く。空白を含む値は `'...'` で囲む。

```
scripts/osc7501.sh <state> [-a APP] [-k KIND] [-p 0〜100] [-t TITLE] [-m MSG]
  state  idle | working | done | blocked | error
  KIND   permission | question | auth（blocked のときだけ）
```

- 名前は `title` → タブ名 → `app` の順に決まる。タブ名や `app` を検証するとき以外は `-t` で付け、ペインの `name` と揃える。
- msg は、既定では `blocked` / `error` のときだけ表示される（`msg_mode`）。
- `app_filter` や `show_app_icon` を使うときは `-a claude-code` を付ける（アイコンがあるのは `claude-code` だけ）。
- `-n` は付けない。送信元が終わるとレコードが消える。
- 状態が変わり続ける様子を見るときは `scripts/osc7501-demo.sh <間隔> <app>` を使う。一度だけ変えるときは、シェルのペインで `scripts/osc7501.sh` を実行する手順を書く。
- 4 件を超えるなら、`split_direction="vertical"` の行を増やす。

### 設定の見比べ

同じエージェントを設定違いで見比べるときは、プラグインの行を縦に並べ、何行目がどの設定かをコメントと報告の両方に書く。

```kdl
    // 1 行目：既定
    pane size=1 borderless=true {
        plugin location="file:target/wasm32-wasip1/debug/zellij-agent-status.wasm"
    }
    // 2 行目：badge_shape "round"
    pane size=1 borderless=true {
        plugin location="file:target/wasm32-wasip1/debug/zellij-agent-status.wasm" {
            badge_shape "round"
        }
    }
```

クリックや選択モードを検証するシーンでは、プラグインは 1 タブに 1 行だけにする。同じタブに複数あると、すべてが選択モードに入ってしまう。

### 幅

ウィンドウの幅を変える手順でもよいが、決まった桁数で見るときはプラグインの行を横に分ける。

```kdl
    // 左：40 桁、右：残り
    pane size=1 split_direction="vertical" {
        pane size=40 borderless=true {
            plugin location="file:target/wasm32-wasip1/debug/zellij-agent-status.wasm"
        }
        pane borderless=true
    }
```

### cwd

`show_cwd` の検証では、エージェントのペインに `cwd="src"` のように付け、スクリプトのパスを `../scripts/osc7501.sh` に直す。

### タブをまたぐ検証

`layout` の中身を次の形に替える。開き直し用の `bash` ペインは置かない（開き直すとタブが増えるため）。

```kdl
layout {
    default_tab_template {
        pane size=1 borderless=true {
            plugin location="zellij:tab-bar"
        }
        children
        pane size=1 borderless=true {
            plugin location="file:target/wasm32-wasip1/debug/zellij-agent-status.wasm"
        }
        pane size=2 borderless=true {
            plugin location="zellij:status-bar"
        }
    }
    tab name="one" focus=true {
        pane command="bash" name="one" {
            args "-c" "scripts/osc7501.sh working"
        }
        pane focus=true
    }
    tab name="two" {
        pane command="bash" name="two" {
            args "-c" "scripts/osc7501.sh blocked -k question -m 'どちらにしますか？'"
        }
    }
}
```

## 報告の形

次の形だけを出す。検証タスクは全シーンを 1 つの表にまとめ、シーンごとの見出しやチェックボックスは使わない。

````markdown
```sh
zellij --layout tmp/<トピック>/01-<シーン>.kdl
zellij --layout tmp/<トピック>/02-<シーン>.kdl
```

| # | kdl | 操作 | 期待する表示 |
|---|---|---|---|
| 1 | 01 | 起動する | 2 行目のバッジの端が半円になる |
| 2 | 01 | `two` をクリックする | `two` のペインにフォーカスが移る |
| 3 | 02 | ... | ... |
````

- 期待する表示は、見れば合否を決められるように具体的に書く（「正しく表示される」とは書かない）。実装から決め、はっきりしないものは断定せず「要判断」と書く。
- kdl では検証できない項目があるときだけ、表の後に 1〜2 行で書く。

ここまで出したら終わる。結果は人間からの報告を待ち、問題があったと言われてから対応する。
