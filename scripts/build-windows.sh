#!/bin/bash
# Cross-compila AudioBridge.exe (Windows x64) no Mac. Requer: brew install mingw-w64 cmake
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
rustup target add x86_64-pc-windows-gnu
cargo build --release --bin audiobridge-windows --target x86_64-pc-windows-gnu
mkdir -p dist
cp "${CARGO_TARGET_DIR:-target}/x86_64-pc-windows-gnu/release/audiobridge-windows.exe" dist/AudioBridge.exe
(cd dist && rm -f AudioBridge-Windows.zip && zip -q AudioBridge-Windows.zip AudioBridge.exe)
echo "OK: dist/AudioBridge.exe"
