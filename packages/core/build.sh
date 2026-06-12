#!/usr/bin/env bash
# Build @vietgs03/layra: wasm-pack the engine, assemble dist/.
set -euo pipefail
cd "$(dirname "$0")"

wasm-pack build ../../crates/layra-wasm --target web --release \
  --out-dir ../../packages/core/dist/wasm --no-typescript --no-pack

mkdir -p dist
cp src/index.js dist/
cp src/index.d.ts dist/
rm -f dist/wasm/.gitignore

echo "built → dist/ ($(du -sh dist | cut -f1))"
