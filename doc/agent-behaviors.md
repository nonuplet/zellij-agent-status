# エージェントごとの挙動

各エージェントが OSC 7501 をいつ・どの状態で送るかと、それをこのプラグインで確かめる方法。
確認は `zellij --layout dev.kdl` で起動し、中央のペインでエージェントを動かして行う。

## Claude Code

- 最新版（2026-11 時点）は OSC 7501 を自分で送る。hooks などの設定は不要。
- `app` は `claude-code`。設定の `apps "claude-code"` で絞り込める。
- Claude Code を終了してシェルに戻ると、約 2 秒で表示が消える（Zellij の挙動。[osc7501-findings.md](osc7501-findings.md) の「レコードが消えるタイミング」）。

### 状態と、その状況の起こし方

2026-11 に実機で確認した。

| 状態 | 送られる状況 | 起こし方の例 |
|---|---|---|
| idle | 起動直後 | `claude` を起動する |
| working | 応答を作っている間 | `1 から 100 までの数それぞれについて、素数かどうかを理由付きで 1 行ずつ書いて` |
| done | 応答が終わった後 | 上の指示の応答が終わるまで待つ |
| blocked / permission | 許可ダイアログが出ている間 | `touch /tmp/zas-test を実行して` |
| blocked / question | 質問が出ている間 | `AskUserQuestion ツールを使って、好きな色を 3 択で質問して` |
| error | API エラーが起きたとき | `claude --model no-such-model` で起動して `こんにちは` を送る |

- 許可ダイアログで許可しても拒否しても、blocked のまま残らない。
- 応答中に Esc で中断しても、working のまま残らない。
