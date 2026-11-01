#!/bin/bash
# Gera AudioBridge.app (Apple Silicon, macOS 15+) e AudioBridge-Mac.zip
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
OUT="dist"; APP="$OUT/AudioBridge.app"
export MACOSX_DEPLOYMENT_TARGET=15.0
cargo build --release --bin audiobridge-mac --target aarch64-apple-darwin
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "${CARGO_TARGET_DIR:-target}/aarch64-apple-darwin/release/audiobridge-mac" "$APP/Contents/MacOS/AudioBridge"
cat > "$APP/Contents/Info.plist" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
 <key>CFBundleName</key><string>AudioBridge</string>
 <key>CFBundleDisplayName</key><string>AudioBridge</string>
 <key>CFBundleIdentifier</key><string>local.audiobridge.mac</string>
 <key>CFBundleExecutable</key><string>AudioBridge</string>
 <key>CFBundlePackageType</key><string>APPL</string>
 <key>CFBundleVersion</key><string>1.0</string>
 <key>CFBundleShortVersionString</key><string>1.0</string>
 <key>LSMinimumSystemVersion</key><string>15.0</string>
 <key>NSHighResolutionCapable</key><true/>
 <key>LSUIElement</key><true/>
 <key>NSLocalNetworkUsageDescription</key><string>O AudioBridge encontra o PC com Windows na sua rede local para receber o áudio.</string>
 <key>NSBonjourServices</key><array><string>_abridge._udp</string><string>_abridgerx._udp</string></array>
</dict></plist>
PL
codesign --force --deep --sign - "$APP"
(cd "$OUT" && rm -f AudioBridge-Mac.zip && ditto -c -k --keepParent AudioBridge.app AudioBridge-Mac.zip)
echo "OK: $APP"
