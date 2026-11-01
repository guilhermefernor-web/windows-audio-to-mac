# AudioBridge

Stream **everything playing on your Windows PC to your Mac**, in real time, over your local network.
Open the app on both machines and it just works: no accounts, no cloud, no IP addresses, no command line.

- **Windows** captures all system audio (WASAPI loopback) and sends it.
- **Mac** (Apple Silicon, macOS 15+) receives it and plays it on whatever output is selected: speakers, headphones, AirPods.
- Low latency (typically 60–100 ms) with [Opus](https://opus-codec.org/) at 48 kHz stereo.
- Finds the other machine automatically (Bonjour/mDNS) and reconnects by itself.
- Starts with the computer and lives quietly in the background (system tray on Windows, no Dock icon on Mac).

## Install

Download both files from the [latest release](../../releases/latest).

**Windows 10/11 (x64)** — `AudioBridge-Windows.zip`
1. Unzip and run `AudioBridge.exe`.
2. On first run Windows asks for administrator permission (UAC) to add a Firewall rule for the app. Accept it; without it the Mac can't reach the PC.
3. SmartScreen may warn because the app is not code-signed: *More info → Run anyway*.

**macOS 15+ (Apple Silicon)** — `AudioBridge-Mac.zip`
1. Unzip and drag `AudioBridge.app` to **Applications**, then open it.
2. If macOS blocks it (the app is not notarized): right-click → **Open** → **Open**.
3. Allow **local network access** when asked. No microphone or screen-recording permission is needed.

## Use

Open AudioBridge once on each machine. That's it.

| | Windows | Mac |
|---|---|---|
| Starts | Streaming immediately | Connects to the PC automatically |
| Close window | Hides to the tray (still running) | Hides, no Dock icon (still running) |
| Open window again | Click the tray icon | Open the AudioBridge app |
| Quit completely | Right-click tray icon → **Quit** | Open the window, then ⌘Q |
| Run at login | On by default, checkbox in the window | On by default, checkbox in the window |

The Mac has a volume slider and a Connect/Disconnect button; the Windows app has a Pause/Start button.
If the Mac can't find the PC (some guest or "client isolation" Wi-Fi networks block discovery), expand
*"Can't find it? Enter the PC's IP"* on the Mac and type the IP shown in the Windows window.

Tips: use 5 GHz Wi-Fi or Ethernet, and set the Windows output format to 48000 Hz
(Sound settings → device → Advanced) to avoid sample-rate conversion.

## How it works

```
Windows                                                  Mac
WASAPI loopback ─► mix/resample to 48 kHz stereo         UDP ─► Opus decode ─► jitter buffer ─► CoreAudio
   (cpal)            ─► Opus (10 ms frames, 160 kbps) ─► UDP ─►   (PLC on loss)  (adaptive, 40–160 ms)
```

- **Discovery:** mDNS service types `_abridge._udp` (PC) and `_abridgerx._udp` (Mac).
- **Transport:** plain UDP, one datagram per 10 ms Opus frame (UDP port 47800 on the PC). The Mac subscribes every
  500 ms and the PC answers with an ack, so either side can come and go and the link recovers on its own.
- **Resilience:** sequence numbers, Opus packet-loss concealment, an adaptive jitter buffer that grows after glitches
  and shrinks when stable, and gentle playback-rate correction to absorb clock drift between the two machines.
- **Devices:** follows the default output device on both sides if you switch speakers or headphones.
- **Single instance:** a loopback-only TCP port (47801) lets a second launch bring up the existing window.
- Written in Rust: [`cpal`](https://crates.io/crates/cpal), [`opus`](https://crates.io/crates/opus),
  [`mdns-sd`](https://crates.io/crates/mdns-sd), [`eframe/egui`](https://crates.io/crates/eframe),
  [`tray-icon`](https://crates.io/crates/tray-icon).

## Limitations

- **No encryption or authentication.** Audio is sent unencrypted and any device on the same LAN that speaks the
  protocol can subscribe. Use it on networks you trust.
- One receiver at a time per PC. Local network only (no internet streaming).
- Windows → Mac only. Not signed or notarized, hence the first-run warnings above.
- Audio at sample rates other than 48 kHz is resampled with simple linear interpolation.
- When nothing is playing on Windows, WASAPI delivers no data; the Mac simply plays silence until sound resumes.

## Build from source

Requires [Rust](https://rustup.rs). From the repo root:

```bash
# Mac app (on macOS, Apple Silicon): dist/AudioBridge.app + zip
./scripts/build-mac.sh

# Windows exe: on Windows
cargo build --release --bin audiobridge-windows
# ...or cross-compile from a Mac (needs: brew install mingw-w64 cmake)
./scripts/build-windows.sh
```

`cargo run --release --bin testsend` streams a test tone and `testrecv` is a headless receiver, which is handy for
trying the network path without WASAPI. GitHub Actions (`.github/workflows/build.yml`) builds both apps on every push.

## Troubleshooting

- **Mac says "Searching…":** check the Windows Firewall rule (Private networks) and that both machines are on the same
  network/VLAN; try the manual IP field.
- **Crackling or dropouts:** move to 5 GHz/Ethernet; the buffer will raise itself, but a weak link still limits quality.
- **No sound:** make sure the Windows app shows "Streaming to …" and the Mac volume slider is up.
