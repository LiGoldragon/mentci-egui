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

use mentci_lib::connection::driver::{
    spawn_driver, ConnectionHandle, DaemonRole, DriverCmd,
};
use mentci_lib::connection::DaemonStatus;
use mentci_lib::{Cmd, EngineEvent, UserEvent, WorkbenchState};
use std::path::PathBuf;
use tokio::runtime::Runtime;

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
}

impl MentciEguiApp {
    pub fn new(principal: signal::Slot, tokio_runtime: Runtime) -> Self {
        Self {
            workbench: WorkbenchState::new(principal),
            pending_cmds: Vec::new(),
            criome_handle: None,
            nexus_handle: None,
            tokio_runtime,
            bootstrap_done: false,
        }
    }

    /// First-frame bootstrap — auto-attempt both connections
    /// so the user sees the lifecycle without having to click.
    fn bootstrap_if_needed(&mut self) {
        if self.bootstrap_done {
            return;
        }
        self.bootstrap_done = true;
        self.pending_cmds.push(Cmd::ConnectCriome);
        self.pending_cmds.push(Cmd::ConnectNexus);
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
                    let _ = h.cmds_tx.send(DriverCmd::SendFrame(frame));
                } else {
                    return Err(crate::error::Error::Lib(
                        mentci_lib::Error::CriomeDisconnected,
                    ));
                }
            }
            Cmd::SendNexus { frame } => {
                if let Some(h) = self.nexus_handle.as_ref() {
                    let _ = h.cmds_tx.send(DriverCmd::SendFrame(frame));
                } else {
                    return Err(crate::error::Error::Lib(
                        mentci_lib::Error::NexusDisconnected,
                    ));
                }
            }
            Cmd::Subscribe { .. }
            | Cmd::Unsubscribe { .. }
            | Cmd::RenderViaNexus { .. }
            | Cmd::SetTimer { .. } => {
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
