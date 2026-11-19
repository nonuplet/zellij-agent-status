#!/usr/bin/env bash
file=$(jq -r '.tool_input.file_path // empty')
[[ "$file" == *.rs ]] || exit 0
cd "$CLAUDE_PROJECT_DIR" && cargo fmt --all