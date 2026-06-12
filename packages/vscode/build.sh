#!/usr/bin/env bash
# Copy the WASM bundle into media/ for the webview.
set -euo pipefail
cd "$(dirname "$0")"

wasm-pack build ../../crates/layra-wasm --target web --release \
  --out-dir ../../packages/vscode/media --no-typescript --no-pack
rm -f media/.gitignore
echo "vscode extension media ready ($(du -sh media | cut -f1))"
