//! `eframe::App` implementation for the daemon-connected Mentci client.
//!
//! The GUI talks to `mentci-daemon` over `signal-mentci`, renders typed
//! request/reply values as NOTA, and keeps no independent approval model.

use std::sync::mpsc;

use crate::daemon_client::{DaemonClient, DaemonTranscriptEntry};

pub struct MentciEguiApp {
    tokio_runtime: tokio::runtime::Runtime,
    bootstrap_done: bool,
    daemon_client: DaemonClient,
    daemon_transcript: Vec<DaemonTranscriptEntry>,
    daemon_replies: mpsc::Receiver<crate::error::Result<DaemonTranscriptEntry>>,
    daemon_reply_sender: mpsc::Sender<crate::error::Result<DaemonTranscriptEntry>>,
    ordinary_request_in_flight: bool,
}

impl MentciEguiApp {
    pub fn new(tokio_runtime: tokio::runtime::Runtime) -> Self {
        let (daemon_reply_sender, daemon_replies) = mpsc::channel();
        Self {
            tokio_runtime,
            bootstrap_done: false,
            daemon_client: DaemonClient::from_environment(),
            daemon_transcript: Vec::new(),
            daemon_replies,
            daemon_reply_sender,
            ordinary_request_in_flight: false,
        }
    }

    fn bootstrap_if_needed(&mut self) {
        if self.bootstrap_done {
            return;
        }
        self.bootstrap_done = true;
        self.request_interface_state();
    }

    fn request_interface_state(&mut self) {
        if self.ordinary_request_in_flight {
            return;
        }
        self.ordinary_request_in_flight = true;
        let sender = self.daemon_reply_sender.clone();
        let client = self.daemon_client.clone();
        self.tokio_runtime.spawn_blocking(move || {
            let _ = sender.send(client.observe_interface_state());
        });
    }

    fn show_meta_mode(&mut self) {
        self.daemon_transcript
            .push(self.daemon_client.meta_mode_placeholder());
    }

    fn drain_daemon_replies(&mut self) {
        while let Ok(result) = self.daemon_replies.try_recv() {
            self.ordinary_request_in_flight = false;
            match result {
                Ok(entry) => self.daemon_transcript.push(entry),
                Err(error) => self.daemon_transcript.push(DaemonTranscriptEntry {
                    mode: crate::daemon_client::DaemonMode::Ordinary,
                    operation: "ObserveInterfaceState".to_string(),
                    socket_path: self.daemon_client.ordinary_socket().clone(),
                    request_nota: "(ObserveInterfaceState ...)".to_string(),
                    reply_nota: format!("{error}"),
                }),
            }
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("mentci");
            ui.separator();
            ui.label("ordinary");
            ui.monospace(self.daemon_client.ordinary_socket().display().to_string());
            ui.separator();
            ui.label("meta");
            ui.monospace(self.daemon_client.meta_socket().display().to_string());
            ui.separator();
            if ui
                .add_enabled(
                    !self.ordinary_request_in_flight,
                    egui::Button::new("observe"),
                )
                .clicked()
            {
                self.request_interface_state();
            }
            if ui.button("meta").clicked() {
                self.show_meta_mode();
            }
            if self.ordinary_request_in_flight {
                ui.spinner();
            }
        });
    }

    fn render_transcript(&self, ui: &mut egui::Ui) {
        if self.daemon_transcript.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("waiting for mentci-daemon");
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for entry in self.daemon_transcript.iter().rev() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(entry.mode.label());
                            ui.label(&entry.operation);
                            ui.monospace(entry.socket_path.display().to_string());
                        });
                        ui.collapsing("request NOTA", |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut entry.request_nota.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                        ui.collapsing("reply NOTA", |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut entry.reply_nota.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .interactive(false),
                            );
                        });
                    });
                    ui.add_space(8.0);
                }
            });
    }
}

impl eframe::App for MentciEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.bootstrap_if_needed();
        self.drain_daemon_replies();

        egui::TopBottomPanel::top("daemon_header").show(ctx, |ui| {
            self.render_header(ui);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_transcript(ui);
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
