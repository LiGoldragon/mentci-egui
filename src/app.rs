//! `eframe::App` impl wrapping [`mentci_lib::WorkbenchState`].
//!
//! Each frame:
//!
//! 1. Drain pending [`mentci_lib::EngineEvent`]s from the two
//!    connection drivers; feed each into
//!    [`mentci_lib::WorkbenchState::on_engine_event`]; collect
//!    the returned [`mentci_lib::Cmd`]s.
//! 2. Derive the per-frame [`mentci_lib::WorkbenchView`] via
//!    [`mentci_lib::WorkbenchState::view`].
//! 3. Paint it via [`crate::render::workbench`]; the render
//!    layer captures user gestures and emits
//!    [`mentci_lib::UserEvent`]s back.
//! 4. For each captured [`mentci_lib::UserEvent`], call
//!    [`mentci_lib::WorkbenchState::on_user_event`]; collect
//!    the returned [`mentci_lib::Cmd`]s.
//! 5. Execute all collected [`mentci_lib::Cmd`]s — spawn /
//!    drop driver tasks, send signal frames on the right
//!    socket, etc.
//!
//! The shell is the runtime; the library is the model.

use mentci_lib::connection::DaemonStatus;
use mentci_lib::connection::driver::{ConnectionHandle, DaemonRole, DriverCmd, spawn_driver};
use mentci_lib::{Cmd, EngineEvent, UserEvent, WorkbenchState};
use std::path::PathBuf;
use std::sync::mpsc;
use tokio::runtime::Runtime;

use crate::daemon_client::{DaemonClient, DaemonTranscriptEntry};

/// Default UDS path for criome's signal listener.
const CRIOME_SOCKET: &str = "/tmp/criome.sock";
/// Default UDS path for nexus-daemon's signal listener.
const NEXUS_SOCKET: &str = "/tmp/nexus.sock";

pub struct MentciEguiApp {
    pub workbench: WorkbenchState,
    /// Pending Cmds to dispatch this frame.
    pub pending_cmds: Vec<Cmd>,
    /// Live connection driver handles. `None` while
    /// disconnected.
    pub criome_handle: Option<ConnectionHandle>,
    pub nexus_handle: Option<ConnectionHandle>,
    /// tokio runtime owns the worker threads on which driver
    /// tasks run. Held by the App so it lives as long as the
    /// window; dropped on app exit.
    pub tokio_runtime: Runtime,
    /// Has the auto-connect logic run yet?
    pub bootstrap_done: bool,
    pub daemon_client: DaemonClient,
    pub daemon_transcript: Vec<DaemonTranscriptEntry>,
    pub daemon_replies: mpsc::Receiver<crate::error::Result<DaemonTranscriptEntry>>,
    pub daemon_reply_sender: mpsc::Sender<crate::error::Result<DaemonTranscriptEntry>>,
    pub ordinary_request_in_flight: bool,
}

impl MentciEguiApp {
    pub fn new(principal: signal::Slot<signal::Principal>, tokio_runtime: Runtime) -> Self {
        let (daemon_reply_sender, daemon_replies) = mpsc::channel();
        Self {
            workbench: WorkbenchState::new(principal),
            pending_cmds: Vec::new(),
            criome_handle: None,
            nexus_handle: None,
            tokio_runtime,
            bootstrap_done: false,
            daemon_client: DaemonClient::from_environment(),
            daemon_transcript: Vec::new(),
            daemon_replies,
            daemon_reply_sender,
            ordinary_request_in_flight: false,
        }
    }

    /// First-frame bootstrap — ask the real Mentci daemon for
    /// a full interface-state projection so the GUI starts as
    /// a daemon client rather than a blank shell.
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

    fn render_daemon_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("mentci_daemon")
            .resizable(true)
            .default_height(220.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("mentci daemon");
                    ui.label(self.daemon_client.ordinary_socket().display().to_string());
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
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.daemon_transcript.is_empty() {
                        ui.label("(waiting for daemon reply)");
                    }
                    for entry in self.daemon_transcript.iter().rev().take(16) {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.strong(entry.mode.label());
                                ui.label(&entry.operation);
                                ui.label(entry.socket_path.display().to_string());
                            });
                            ui.collapsing("request NOTA", |ui| {
                                ui.monospace(&entry.request_nota);
                            });
                            ui.collapsing("reply NOTA", |ui| {
                                ui.monospace(&entry.reply_nota);
                            });
                        });
                    }
                });
            });
    }

    /// Drain any engine events queued by the driver tasks
    /// since the last frame.
    fn drain_engine_events(&mut self) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        if let Some(h) = self.criome_handle.as_mut() {
            while let Ok(ev) = h.events_rx.try_recv() {
                events.push(ev);
            }
        }
        if let Some(h) = self.nexus_handle.as_mut() {
            while let Ok(ev) = h.events_rx.try_recv() {
                events.push(ev);
            }
        }
        events
    }

    /// Execute one Cmd. Spawns / drops drivers; routes
    /// outbound signal frames to the right socket.
    pub fn execute_cmd(&mut self, cmd: Cmd) -> crate::error::Result<()> {
        match cmd {
            Cmd::ConnectCriome => {
                self.workbench.connections.criome.status = DaemonStatus::Connecting;
                let handle = spawn_driver(
                    self.tokio_runtime.handle(),
                    PathBuf::from(CRIOME_SOCKET),
                    DaemonRole::Criome,
                );
                self.criome_handle = Some(handle);
            }
            Cmd::ConnectNexus => {
                self.workbench.connections.nexus.status = DaemonStatus::Connecting;
                let handle = spawn_driver(
                    self.tokio_runtime.handle(),
                    PathBuf::from(NEXUS_SOCKET),
                    DaemonRole::Nexus,
                );
                self.nexus_handle = Some(handle);
            }
            Cmd::DisconnectCriome => {
                if let Some(h) = self.criome_handle.as_ref() {
                    let _ = h.cmds_tx.send(DriverCmd::Disconnect);
                }
                // Drop after asking the driver to wind down so
                // the goodbye disconnect event has a chance to
                // arrive.
                self.criome_handle = None;
            }
            Cmd::DisconnectNexus => {
                if let Some(h) = self.nexus_handle.as_ref() {
                    let _ = h.cmds_tx.send(DriverCmd::Disconnect);
                }
                self.nexus_handle = None;
            }
            Cmd::SendCriome { frame } => {
                if let Some(h) = self.criome_handle.as_ref() {
                    let _ = h.cmds_tx.send(DriverCmd::SendFrame(Box::new(frame)));
                } else {
                    return Err(crate::error::Error::Lib(
                        mentci_lib::Error::CriomeDisconnected,
                    ));
                }
            }
            Cmd::SendNexus { frame } => {
                if let Some(h) = self.nexus_handle.as_ref() {
                    let _ = h.cmds_tx.send(DriverCmd::SendFrame(Box::new(frame)));
                } else {
                    return Err(crate::error::Error::Lib(
                        mentci_lib::Error::NexusDisconnected,
                    ));
                }
            }
            Cmd::NotifyApproval { .. }
            | Cmd::PublishApprovalUpdates { .. }
            | Cmd::ConfirmApprovalSubscription { .. }
            | Cmd::ConfirmApprovalUnsubscription { .. }
            | Cmd::SubmitApproval { .. } => {
                // The TUI/status-bar/email clients will handle these
                // when the approval surface is wired. The shell still
                // accepts the commands so the library model can advance.
            }
            Cmd::RenderViaNexus { .. } | Cmd::SetTimer { .. } => {
                // Real wiring lands as the corresponding wire
                // verbs are exercised end-to-end. For now, drop
                // silently — the model has already updated.
            }
        }
        Ok(())
    }
}

impl eframe::App for MentciEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 0. First-frame bootstrap.
        self.bootstrap_if_needed();
        self.drain_daemon_replies();

        // 1. Drain engine events.
        let events = self.drain_engine_events();
        for ev in events {
            let cmds = self.workbench.on_engine_event(ev);
            self.pending_cmds.extend(cmds);
        }

        // 2. Derive view.
        let view = self.workbench.view();

        // 3. Paint, capturing user gestures.
        let mut user_events: Vec<UserEvent> = Vec::new();
        crate::render::workbench(ctx, &view, &mut user_events);
        self.render_daemon_panel(ctx);

        // 4. Apply each captured user event.
        for ev in user_events {
            let cmds = self.workbench.on_user_event(ev);
            self.pending_cmds.extend(cmds);
        }

        // 5. Dispatch pending cmds.
        let cmds = std::mem::take(&mut self.pending_cmds);
        for cmd in cmds {
            if let Err(e) = self.execute_cmd(cmd) {
                eprintln!("cmd dispatch failed: {e}");
            }
        }

        // Re-poll soon so engine events arriving between
        // input frames also surface promptly.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}
