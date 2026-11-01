//! AudioBridge para Windows: captura todo o áudio do PC (WASAPI loopback) e envia ao Mac.
#![cfg_attr(windows, windows_subsystem = "windows")]

use audiobridge::sender::Sender;
use eframe::egui;
use std::time::Duration;

struct App {
    tx: Sender,
    ip: Option<String>,
    autostart: bool,
    #[cfg(windows)]
    _tray: Option<tray_icon::TrayIcon>,
}

#[cfg(windows)]
fn make_tray() -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuEvent, MenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    // 32x32 blue disc with a white "sound wave" bar pattern, drawn in code (no asset files).
    let n = 32usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            if dx * dx + dy * dy <= 15.0 * 15.0 {
                let bar = (x >= 7 && x < 25) && ((x - 7) % 4 < 2) && {
                    let h = [4, 8, 12, 8, 14, 6, 10, 4, 6][(x - 7) / 2 % 9] as f32;
                    dy.abs() <= h / 1.2
                };
                let c: [u8; 4] = if bar { [255, 255, 255, 255] } else { [38, 110, 220, 255] };
                rgba[(y * n + x) * 4..][..4].copy_from_slice(&c);
            }
        }
    }
    let menu = Menu::new();
    let open = MenuItem::with_id("open", "Abrir AudioBridge", true, None);
    let quit = MenuItem::with_id("quit", "Encerrar", true, None);
    menu.append_items(&[&open, &quit]).ok()?;

    MenuEvent::set_event_handler(Some(|e: MenuEvent| match e.id.0.as_str() {
        "open" => audiobridge::shell::request_show(),
        "quit" => std::process::exit(0),
        _ => {}
    }));
    TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }
        | TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = e
        {
            audiobridge::shell::request_show();
        }
    }));
    TrayIconBuilder::new()
        .with_icon(Icon::from_rgba(rgba, n as u32, n as u32).ok()?)
        .with_tooltip("AudioBridge")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .ok()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        audiobridge::shell::update(ui.ctx());
        ui.ctx().request_repaint_after(Duration::from_millis(250));
        let mac = self.tx.connected_to();
        let found = self.tx.receivers_found();
        let streaming = self.tx.is_streaming();
        let green = egui::Color32::from_rgb(40, 180, 90);
        let amber = egui::Color32::from_rgb(230, 170, 30);

        let (dot, text) = match (&mac, streaming) {
            (Some(m), true) => (green, format!("Transmitindo para {m}")),
            (Some(m), false) => (amber, format!("Mac conectado: {m}. Clique em Iniciar.")),
            (None, true) if !found.is_empty() => (amber, format!("Mac encontrado: {}. Conectando…", found.join(", "))),
            (None, true) => (amber, "Procurando o Mac…".to_string()),
            (None, false) => (egui::Color32::GRAY, "Transmissão pausada.".to_string()),
        };

        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(6.0);
            ui.heading("AudioBridge");
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(dot, "●");
                ui.label(text);
            });
            if let Some(e) = self.tx.error() {
                ui.colored_label(egui::Color32::from_rgb(210, 70, 70), e);
            }
            ui.add_space(12.0);
            let label = if streaming { "Pausar transmissão" } else { "Iniciar transmissão" };
            if ui.add_sized([ui.available_width(), 42.0], egui::Button::new(label)).clicked() {
                if streaming { self.tx.stop() } else { self.tx.start() }
            }
            ui.add_space(10.0);
            if ui.checkbox(&mut self.autostart, "Abrir ao ligar o Windows").changed() {
                audiobridge::autostart::set_enabled(self.autostart);
            }
            ui.add_space(6.0);
            ui.weak("Fechar a janela mantém o AudioBridge rodando na bandeja (ícones ocultos). Para encerrar: botão direito no ícone → Encerrar.");
            if let Some(ip) = &self.ip {
                ui.weak(format!("IP deste PC: {ip}"));
            }
        });
    }
}

fn main() -> eframe::Result {
    if !audiobridge::single::acquire(audiobridge::shell::request_show) {
        return Ok(());
    }
    audiobridge::autostart::init();
    let tx = Sender::new().expect("não foi possível abrir a rede");
    tx.start(); // automatic: stream as soon as the app opens; the Mac connects by itself
    let ip = audiobridge::proto::local_ip();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([380.0, 300.0])
            .with_resizable(false)
            .with_visible(!audiobridge::shell::started_in_background()),
        ..Default::default()
    };
    eframe::run_native(
        "AudioBridge",
        opts,
        Box::new(|cc| {
            audiobridge::shell::init(&cc.egui_ctx);
            Ok(Box::new(App {
                tx,
                ip,
                autostart: audiobridge::autostart::is_enabled(),
                #[cfg(windows)]
                _tray: make_tray(),
            }))
        }),
    )
}
