#!/bin/bash
# Build everything: CLI, Tauri app, and install
set -e

export CXXFLAGS="-I$(xcrun --show-sdk-path)/usr/include/c++/v1"

echo "=== Building CLI (release) ==="
cargo build --release -p minutes-cli

echo "=== Building Tauri app ==="
cargo tauri build --bundles app

echo "=== Installing CLI ==="
mkdir -p ~/.local/bin
cp -f target/release/minutes ~/.local/bin/minutes && echo "  Installed to ~/.local/bin/"
# Also try homebrew cellar if it exists
CELLAR="/opt/homebrew/Cellar/minutes/0.1.0/bin"
if [ -d "$CELLAR" ]; then
    cp -f target/release/minutes "$CELLAR/minutes" 2>/dev/null || true
fi

echo ""
echo "=== Done ==="
echo "  CLI:  $(which minutes) — $(minutes --version 2>&1)"
echo "  App:  target/release/bundle/macos/Minutes.app"
echo ""
echo "  Launch: open target/release/bundle/macos/Minutes.app"
