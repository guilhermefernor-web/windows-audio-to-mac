//! AudioBridge para Windows: captura todo o áudio do PC (WASAPI loopback) e envia ao Mac.
#![cfg_attr(windows, windows_subsystem = "windows")]

use audiobridge::sender::Sender;
use eframe::egui;
use std::time::Duration;

struct App {
    tx: Sender,
    ip: Option<String>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint_after(Duration::from_millis(250));
        let mac = self.tx.connected_to();
        let found = self.tx.receivers_found();
        let streaming = self.tx.is_streaming();
        let green = egui::Color32::from_rgb(40, 180, 90);
        let amber = egui::Color32::from_rgb(230, 170, 30);

        let (dot, text) = match (&mac, streaming) {
            (Some(m), true) => (green, format!("Transmitindo para {m}")),
            (Some(m), false) => (amber, format!("Mac conectado: {m}. Clique em Iniciar.")),
            (None, true) => (amber, "Transmitindo… aguardando o Mac conectar".to_string()),
            (None, false) if !found.is_empty() => {
                (egui::Color32::GRAY, format!("Mac encontrado: {}. Clique em Conectar no Mac.", found.join(", ")))
            }
            (None, false) => (egui::Color32::GRAY, "Procurando o Mac…".to_string()),
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
            let label = if streaming { "Parar transmissão" } else { "Iniciar transmissão" };
            if ui.add_sized([ui.available_width(), 42.0], egui::Button::new(label)).clicked() {
                if streaming { self.tx.stop() } else { self.tx.start() }
            }
            ui.add_space(12.0);
            if let Some(ip) = &self.ip {
                ui.weak(format!("IP deste PC: {ip} (só se o Mac não achar sozinho)"));
            }
        });
    }
}

fn main() -> eframe::Result {
    let tx = Sender::new().expect("não foi possível abrir a rede");
    let ip = audiobridge::proto::local_ip();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([380.0, 230.0]).with_resizable(false),
        ..Default::default()
    };
    eframe::run_native("AudioBridge", opts, Box::new(|_cc| Ok(Box::new(App { tx, ip }))))
}
