#!/usr/bin/env bash
# Build the playground: wasm-pack the engine, assemble static site in dist/.
set -euo pipefail
cd "$(dirname "$0")/.."

wasm-pack build crates/layra-wasm --target web --release --out-dir ../../playground/public/pkg --no-typescript --no-pack

rm -rf playground/dist
mkdir -p playground/dist
cp -r playground/public/* playground/dist/
# wasm-pack leaves a .gitignore inside out-dir; drop it from dist
rm -f playground/dist/pkg/.gitignore

echo "playground built → playground/dist ($(du -sh playground/dist | cut -f1))"
