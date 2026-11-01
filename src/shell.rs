//! Window lifecycle shared by both apps: closing hides to the background instead of quitting,
//! and a "show" request (tray click, re-opening the app, second launch) brings the window back.

use eframe::egui;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::OnceLock;

static SHOW: AtomicBool = AtomicBool::new(false);
static BG_FRAMES: AtomicU8 = AtomicU8::new(0);
static CTX: OnceLock<egui::Context> = OnceLock::new();

pub fn started_in_background() -> bool {
    std::env::args().any(|a| a == "--background")
}

/// Call from the app creator closure (main thread).
pub fn init(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
    platform::init(started_in_background());
}

/// Ask for the window to appear. Safe from any thread.
pub fn request_show() {
    SHOW.store(true, Ordering::Relaxed);
    platform::show_now();
    if let Some(c) = CTX.get() {
        c.request_repaint();
    }
}

/// Call at the top of every `update`.
pub fn update(ctx: &egui::Context) {
    // eframe reveals the window after its first frames, so keep it hidden through them when
    // launched at login (`--background`), unless the user already asked for it.
    if started_in_background() && !SHOW.load(Ordering::Relaxed) && BG_FRAMES.load(Ordering::Relaxed) < 8 {
        BG_FRAMES.fetch_add(1, Ordering::Relaxed);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        platform::hide_now();
    }
    if ctx.input(|i| i.viewport().close_requested()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        platform::hide_now();
    }
    if SHOW.swap(false, Ordering::Relaxed) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        platform::show_now();
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2::runtime::{AnyObject, Sel};
    use objc2::{sel, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use std::ffi::{c_char, c_void};

    unsafe extern "C" {
        fn class_addMethod(cls: *const c_void, name: Sel, imp: *const c_void, types: *const c_char) -> bool;
    }

    extern "C" fn should_handle_reopen(_this: *mut AnyObject, _cmd: Sel, _app: *mut AnyObject, _visible: bool) -> bool {
        super::request_show();
        true
    }

    pub fn init(background: bool) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let app = NSApplication::sharedApplication(mtm);
        // Clicking the app icon again while it's hidden sends "reopen"; winit doesn't handle it, so add it.
        if let Some(delegate) = app.delegate() {
            unsafe {
                let obj = &*delegate as *const _ as *const AnyObject;
                let cls = (*obj).class() as *const _ as *const c_void;
                class_addMethod(
                    cls,
                    sel!(applicationShouldHandleReopen:hasVisibleWindows:),
                    should_handle_reopen as *const c_void,
                    c"c@:@c".as_ptr(),
                );
            }
            // AppKit caches which delegate methods exist; re-set it so the new one is seen.
            app.setDelegate(None);
            app.setDelegate(Some(&delegate));
        }
        if background {
            app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
            hide_windows(mtm);
        } else {
            app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        }
    }

    fn hide_windows(mtm: MainThreadMarker) {
        for w in NSApplication::sharedApplication(mtm).windows() {
            w.orderOut(None);
        }
    }

    pub fn hide_now() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        hide_windows(mtm);
        NSApplication::sharedApplication(mtm).setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }

    #[allow(deprecated)]
    pub fn show_now() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        for w in app.windows() {
            w.makeKeyAndOrderFront(None);
        }
        app.activateIgnoringOtherApps(true);
    }
}

#[cfg(windows)]
mod platform {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    fn hwnd() -> *mut core::ffi::c_void {
        let title: Vec<u16> = "AudioBridge\0".encode_utf16().collect();
        unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) }
    }

    pub fn init(background: bool) {
        if background {
            hide_now();
        }
    }

    pub fn hide_now() {
        let h = hwnd();
        if !h.is_null() {
            unsafe { ShowWindow(h, SW_HIDE) };
        }
    }

    pub fn show_now() {
        let h = hwnd();
        if !h.is_null() {
            unsafe {
                ShowWindow(h, SW_SHOW);
                ShowWindow(h, SW_RESTORE);
                SetForegroundWindow(h);
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    pub fn init(_: bool) {}
    pub fn hide_now() {}
    pub fn show_now() {}
}
