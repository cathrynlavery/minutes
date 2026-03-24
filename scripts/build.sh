#!/bin/bash
# Build everything: CLI, Tauri app, and install
set -e

export CXXFLAGS="-I$(xcrun --show-sdk-path)/usr/include/c++/v1"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

# Code signing + notarization (set these to enable signed builds)
export APPLE_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:-Developer ID Application: Mathieu Silverstein (63TMLKT8HN)}"
export APPLE_API_ISSUER="${APPLE_API_ISSUER:-2f202b92-887c-4dfc-b6ef-38b3c6347004}"
export APPLE_API_KEY="${APPLE_API_KEY:-49R7VWVUJ7}"
export APPLE_API_KEY_PATH="${APPLE_API_KEY_PATH:-$HOME/.config/minutes/keys/AuthKey_49R7VWVUJ7.p8}"

echo "=== Building CLI (release) ==="
cargo build --release -p minutes-cli

echo "=== Building calendar helper ==="
swiftc -O \
    -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist \
    -Xlinker scripts/calendar-helper-Info.plist \
    scripts/calendar-events.swift -o target/release/calendar-events
echo "  Built target/release/calendar-events"

echo "=== Building Tauri app ==="
cargo tauri build --bundles app

echo "=== Embedding calendar helper in app bundle ==="
APP_RESOURCES="target/release/bundle/macos/Minutes.app/Contents/Resources"
mkdir -p "$APP_RESOURCES"
cp -f target/release/calendar-events "$APP_RESOURCES/calendar-events"
echo "  Embedded in $APP_RESOURCES/"

echo "=== Installing CLI ==="
mkdir -p ~/.local/bin
cp -f target/release/minutes ~/.local/bin/minutes && echo "  Installed to ~/.local/bin/"
# Also try homebrew cellar if it exists
CELLAR="/opt/homebrew/Cellar/minutes/0.1.0/bin"
if [ -d "$CELLAR" ]; then
    cp -f target/release/minutes "$CELLAR/minutes" 2>/dev/null || true
fi

echo ""

# Install to /Applications if --install flag is passed
if [[ "$*" == *"--install"* ]]; then
    echo "=== Installing app to /Applications ==="
    cp -r target/release/bundle/macos/Minutes.app /Applications/
    echo "  Installed to /Applications/Minutes.app"
fi

echo "=== Done ==="
echo "  CLI:  $(which minutes) — $(minutes --version 2>&1)"
echo "  App:  target/release/bundle/macos/Minutes.app"
echo ""
if [ -d "/Applications/Minutes.app" ]; then
    echo "  Relaunch: open /Applications/Minutes.app"
else
    echo "  Launch: open target/release/bundle/macos/Minutes.app"
    echo "  Install: ./scripts/build.sh --install"
fi
