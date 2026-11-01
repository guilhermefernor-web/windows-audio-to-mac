//! AudioBridge para Mac: recebe o áudio do Windows e toca nas caixas/fones do Mac.
#![cfg_attr(windows, windows_subsystem = "windows")]

use audiobridge::discovery::Discovery;
use audiobridge::proto;
use audiobridge::receiver::{Link, Receiver};
use eframe::egui;
use std::time::Duration;

struct App {
    rx: Receiver,
    disc: Option<Discovery>,
    selected: usize,
    volume: f32,
    /// True after the user presses Disconnect: stops auto-connect until they press Connect.
    manual_off: bool,
    manual_ip: String,
    wanted: Option<String>,
}

impl App {
    fn new() -> Self {
        let rx = Receiver::new().expect("receiver");
        let disc = Discovery::start(rx.name(), proto::SERVICE_RX, rx.port, proto::SERVICE_SRC).ok();
        Self { rx, disc, selected: 0, volume: 0.8, manual_off: false, manual_ip: String::new(), wanted: None }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(Duration::from_millis(250));
        let peers = self.disc.as_ref().map(|d| d.list()).unwrap_or_default();
        let st = self.rx.status();
        self.rx.set_volume(self.volume);
        if self.selected >= peers.len() {
            self.selected = 0;
        }

        // Auto-connect when exactly one streamer is around (or reconnect to the one we used).
        if st.link == Link::Idle && !self.manual_off {
            let pick = match &self.wanted {
                Some(n) => peers.iter().find(|p| &p.name == n),
                None if peers.len() == 1 => peers.first(),
                None => None,
            };
            if let Some(p) = pick {
                self.wanted = Some(p.name.clone());
                self.rx.connect(p.addr);
            }
        }

        let (dot, text) = match st.link {
            Link::Idle if peers.is_empty() => (egui::Color32::GRAY, "Procurando o PC com Windows…".to_string()),
            Link::Idle => (egui::Color32::GRAY, "PC encontrado. Clique em Conectar.".to_string()),
            Link::Searching => (egui::Color32::from_rgb(230, 170, 30), "Conectando…".to_string()),
            Link::Waiting => (
                egui::Color32::from_rgb(230, 170, 30),
                format!("Conectado a {}. Clique em Iniciar no Windows.", st.sender_name),
            ),
            Link::Playing => (egui::Color32::from_rgb(40, 180, 90), format!("Tocando o áudio de {}", st.sender_name)),
        };

        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(6.0);
            ui.heading("AudioBridge");
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(dot, "●");
                ui.label(text);
            });
            if let Some(e) = &st.error {
                ui.colored_label(egui::Color32::from_rgb(210, 70, 70), e);
            }
            ui.add_space(10.0);

            if peers.len() > 1 && st.link == Link::Idle {
                egui::ComboBox::from_label("PC")
                    .selected_text(peers[self.selected].name.clone())
                    .show_ui(ui, |ui| {
                        for (i, p) in peers.iter().enumerate() {
                            ui.selectable_value(&mut self.selected, i, &p.name);
                        }
                    });
                ui.add_space(6.0);
            }

            let connected = st.link != Link::Idle;
            let label = if connected { "Desconectar" } else { "Conectar" };
            if ui.add_sized([ui.available_width(), 38.0], egui::Button::new(label)).clicked() {
                if connected {
                    self.manual_off = true;
                    self.wanted = None;
                    self.rx.disconnect();
                } else if let Some(p) = peers.get(self.selected) {
                    self.manual_off = false;
                    self.wanted = Some(p.name.clone());
                    self.rx.connect(p.addr);
                }
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label("Volume");
                ui.add(egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false));
                ui.label(format!("{}%", (self.volume * 100.0).round() as u32));
            });

            if st.link == Link::Idle && peers.is_empty() {
                ui.add_space(10.0);
                ui.collapsing("Não achou? Digite o IP do PC", |ui| {
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.manual_ip);
                        if ui.button("Conectar").clicked() {
                            let ip = self.manual_ip.trim();
                            let addr = if ip.contains(':') { ip.to_string() } else { format!("{ip}:{}", proto::DEFAULT_PORT) };
                            if let Ok(a) = addr.parse() {
                                self.manual_off = false;
                                self.rx.connect(a);
                            }
                        }
                    });
                });
            }
            if st.link == Link::Playing {
                ui.add_space(8.0);
                ui.weak(format!("Buffer {} ms · perdidos {} · falhas {}", st.buffer_ms, st.lost, st.underruns));
            }
        });
    }
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([380.0, 260.0]).with_resizable(false),
        ..Default::default()
    };
    eframe::run_native("AudioBridge", opts, Box::new(|_cc| Ok(Box::new(App::new()))))
}
