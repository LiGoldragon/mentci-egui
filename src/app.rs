//! `eframe::App` implementation for the daemon-connected Mentci client.
//!
//! The shell is now thin in the intended sense: it owns no approval logic and
//! no per-socket state of its own. It holds a `mentci_lib::ObservationModel`
//! (the shared MVU core, keyed by component socket) and feeds it typed
//! `signal-mentci` replies as `EngineEvent`s; it paints the model's
//! `ObservationView` and renders each reply through `mentci_lib`'s NOTA-fallback
//! renderer. mentci-lib is the application; this file is the rendering.

use std::sync::mpsc;

use mentci_lib::{
    ComponentSocketKind, EngineEvent, ObservationModel, RenderNota, RenderOrigin, RenderedObject,
    UserEvent,
};
use signal_mentci::{InterfaceInterest, MentciReply, SubscriberName};

use crate::daemon_client::{DaemonClient, SocketKind};

/// One rendered observation the shell shows: which operation produced it and
/// the NOTA body the shared renderer produced.
struct ObservationEntry {
    operation: String,
    rendered: RenderedObject,
}

pub struct MentciEguiApp {
    tokio_runtime: tokio::runtime::Runtime,
    bootstrap_done: bool,
    daemon_client: DaemonClient,
    /// The shared model. The shell routes every typed reply through it.
    model: ObservationModel,
    /// Rendered transcript — each entry's body comes from mentci-lib's
    /// `RenderNota`, not a hand-rolled `to_nota()` in this shell.
    entries: Vec<ObservationEntry>,
    daemon_replies: mpsc::Receiver<crate::error::Result<MentciReply>>,
    daemon_reply_sender: mpsc::Sender<crate::error::Result<MentciReply>>,
    ordinary_request_in_flight: bool,
}

impl MentciEguiApp {
    pub fn new(tokio_runtime: tokio::runtime::Runtime) -> Self {
        let (daemon_reply_sender, daemon_replies) = mpsc::channel();
        Self {
            tokio_runtime,
            bootstrap_done: false,
            daemon_client: DaemonClient::from_environment(),
            model: ObservationModel::new(SubscriberName::new("mentci-egui")),
            entries: Vec::new(),
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
        // Register the gesture with the shared model so the socket row shows
        // "connecting" while the request is in flight. The returned Cmd is the
        // request the worker thread will send; the shell's transport executes
        // it (here, the simple blocking DaemonClient).
        let _commands = self.model.on_user_event(UserEvent::Observe {
            socket: ComponentSocketKind::Mentci,
            interest: InterfaceInterest::FullInterfaceState,
        });
        self.ordinary_request_in_flight = true;
        let sender = self.daemon_reply_sender.clone();
        let client = self.daemon_client.clone();
        self.tokio_runtime.spawn_blocking(move || {
            let _ = sender
                .send(client.observe_interface_state_typed(InterfaceInterest::FullInterfaceState));
        });
    }

    fn drain_daemon_replies(&mut self) {
        while let Ok(result) = self.daemon_replies.try_recv() {
            self.ordinary_request_in_flight = false;
            match result {
                Ok(reply) => self.absorb_reply(reply),
                Err(error) => self.entries.push(ObservationEntry {
                    operation: "ObserveInterfaceState (error)".to_string(),
                    rendered: format!("{error}").render_nota(RenderOrigin::Reply),
                }),
            }
        }
    }

    /// Fold a typed reply into the shared model and render it. The shell does
    /// not interpret the reply itself — mentci-lib's model owns the state and
    /// the renderer owns the projection.
    fn absorb_reply(&mut self, reply: MentciReply) {
        let rendered = reply.render_nota(RenderOrigin::Reply);
        if let MentciReply::InterfaceObservationOpened(opened) = &reply {
            self.model.on_engine_event(EngineEvent::ObservationOpened {
                socket: ComponentSocketKind::Mentci,
                opened: opened.clone(),
            });
        }
        self.entries.push(ObservationEntry {
            operation: self.reply_operation_name(&reply).to_string(),
            rendered,
        });
    }

    /// Name the reply variant for the transcript header — a closed match on
    /// the contract enum, no string sniffing.
    fn reply_operation_name(&self, reply: &MentciReply) -> &'static str {
        match reply {
            MentciReply::QuestionPresented(_) => "QuestionPresented",
            MentciReply::UpdateAccepted(_) => "UpdateAccepted",
            MentciReply::InterfaceObservationOpened(_) => "InterfaceObservationOpened",
            MentciReply::VerdictAccepted(_) => "VerdictAccepted",
            MentciReply::AnswerProposalAdmitted(_) => "AnswerProposalAdmitted",
            MentciReply::InterfaceObservationRetracted(_) => "InterfaceObservationRetracted",
            MentciReply::Rejection(_) => "Rejection",
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("mentci");
            ui.separator();
            ui.label(SocketKind::Mentci.label());
            ui.monospace(self.daemon_client.ordinary_socket().display().to_string());
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
            if self.ordinary_request_in_flight {
                ui.spinner();
            }
        });

        // The shared model's view: one row per observed component socket plus
        // the approval summary. The shell paints what the model says.
        let view = self.model.view();
        ui.horizontal_wrapped(|ui| {
            for socket in &view.sockets {
                ui.group(|ui| {
                    ui.label(socket.socket.as_str());
                    ui.label(format!("{:?}", socket.liveness));
                    if let Some(revision) = &socket.revision {
                        ui.monospace(format!("rev {}", revision.value()));
                    }
                });
            }
            ui.separator();
            ui.label(format!(
                "pending {} | answered {}",
                view.approval.pending_count, view.approval.answered_count
            ));
        });
    }

    fn render_transcript(&self, ui: &mut egui::Ui) {
        if self.entries.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("waiting for mentci-daemon");
            });
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for entry in self.entries.iter().rev() {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(entry.rendered.origin().label());
                            ui.label(&entry.operation);
                        });
                        ui.add(
                            egui::TextEdit::multiline(&mut entry.rendered.body())
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .interactive(false),
                        );
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
