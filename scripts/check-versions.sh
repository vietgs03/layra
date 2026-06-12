#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo_v=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
npm_v=$(python3 -c "import json; print(json.load(open('packages/core/package.json'))['version'])")
vsix_v=$(python3 -c "import json; print(json.load(open('packages/vscode/package.json'))['version'])")
if [ "$cargo_v" != "$npm_v" ] || [ "$cargo_v" != "$vsix_v" ]; then
  echo "version drift: cargo=$cargo_v npm=$npm_v vscode=$vsix_v" >&2
  exit 1
fi
echo "versions in sync: $cargo_v"
