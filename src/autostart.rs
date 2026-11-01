//! "Open when the computer starts". Enabled by default on first run; the user can untick it.

use std::path::PathBuf;

fn prefs_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(any(target_os = "macos", windows)))]
    let base: Option<PathBuf> = None;
    let d = base?.join("AudioBridge");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// Call once at startup: turns autostart on the very first time, and repairs the entry if the app moved.
pub fn init() {
    let Some(d) = prefs_dir() else { return };
    let marker = d.join("autostart-initialized");
    if !marker.exists() {
        set_enabled(true);
        let _ = std::fs::write(&marker, "1");
    } else if is_enabled() {
        set_enabled(true); // rewrites with the current app path
    }
}

#[cfg(target_os = "macos")]
fn plist_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/LaunchAgents/local.audiobridge.mac.plist"))
}

#[cfg(target_os = "macos")]
fn app_bundle() -> Option<PathBuf> {
    // .../AudioBridge.app/Contents/MacOS/AudioBridge
    let exe = std::env::current_exe().ok()?;
    let app = exe.parent()?.parent()?.parent()?.to_path_buf();
    (app.extension()? == "app").then_some(app)
}

#[cfg(target_os = "macos")]
pub fn is_enabled() -> bool {
    plist_path().is_some_and(|p| p.exists())
}

#[cfg(target_os = "macos")]
pub fn set_enabled(on: bool) {
    let Some(p) = plist_path() else { return };
    if !on {
        let _ = std::fs::remove_file(p);
        return;
    }
    let Some(app) = app_bundle() else { return };
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
 <key>Label</key><string>local.audiobridge.mac</string>
 <key>ProgramArguments</key><array>
  <string>/usr/bin/open</string><string>-g</string><string>-a</string><string>{}</string>
  <string>--args</string><string>--background</string>
 </array>
 <key>RunAtLoad</key><true/>
</dict></plist>
"#,
        app.display()
    );
    if std::fs::read_to_string(&p).ok().as_deref() != Some(plist.as_str()) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, plist);
    }
}

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
pub fn is_enabled() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>("AudioBridge"))
        .is_ok()
}

#[cfg(windows)]
pub fn set_enabled(on: bool) {
    use winreg::{enums::*, RegKey};
    let Ok((k, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY) else { return };
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let _ = k.set_value("AudioBridge", &format!("\"{}\" --background", exe.display()));
        }
    } else {
        let _ = k.delete_value("AudioBridge");
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn is_enabled() -> bool { false }
#[cfg(not(any(target_os = "macos", windows)))]
pub fn set_enabled(_on: bool) {}
