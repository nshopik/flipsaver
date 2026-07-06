#!/usr/bin/env bash
# Cross-build and drop the .scr where Windows can run it.
set -euo pipefail
DEST="${1:-/mnt/c/Temp}"
cd "$(dirname "$0")/.."
cargo xwin build --release --target x86_64-pc-windows-msvc
mkdir -p "$DEST"
cp target/x86_64-pc-windows-msvc/release/flipsaver.exe "$DEST/flipsaver.scr"
echo "deployed: $DEST/flipsaver.scr"
