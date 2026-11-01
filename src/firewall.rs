//! Windows only: the Mac contacts the PC over UDP, which Windows Firewall blocks until the app is allowed.
//! On first run (and when the .exe moves) ask once, via the normal UAC prompt, to add an inbound rule.

#[cfg(windows)]
pub fn ensure_rule() {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const NO_WINDOW: u32 = 0x0800_0000;

    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.display().to_string();
    let marker = crate::autostart::prefs_dir().map(|d| d.join("firewall-asked"));
    if let Some(m) = &marker {
        if std::fs::read_to_string(m).ok().as_deref() == Some(exe.as_str()) {
            return; // already set up (or the user declined) for this exact file
        }
    }
    if let Some(m) = &marker {
        let _ = std::fs::write(m, &exe);
    }
    let inner = format!(
        "/c netsh advfirewall firewall delete rule name=AudioBridge & netsh advfirewall firewall add rule name=AudioBridge dir=in action=allow program=\"{exe}\" protocol=UDP profile=any enable=yes"
    );
    let ps = format!(
        "Start-Process -FilePath cmd.exe -Verb RunAs -WindowStyle Hidden -ArgumentList '{}'",
        inner.replace('\'', "''")
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
        .creation_flags(NO_WINDOW)
        .spawn();
}

#[cfg(not(windows))]
pub fn ensure_rule() {}
