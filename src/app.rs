//! `eframe::App` impl wrapping [`mentci_lib::WorkbenchState`].
//!
//! Each frame:
//!
//! 1. Drain pending [`mentci_lib::EngineEvent`]s from the
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
//! 5. Execute all collected [`mentci_lib::Cmd`]s — send signal
//!    frames on the appropriate sockets, schedule timers, etc.
//!
//! The shell is the runtime; the library is the model.

use mentci_lib::{Cmd, UserEvent, WorkbenchState};

pub struct MentciEguiApp {
    pub workbench: WorkbenchState,
    /// Pending Cmds to dispatch this frame.
    pub pending_cmds: Vec<Cmd>,
    // Connection drivers (UDS sockets, frame buffers) land
    // when the daemon-side handshake is wired.
}

impl MentciEguiApp {
    pub fn new(principal: signal::Slot) -> Self {
        Self {
            workbench: WorkbenchState::new(principal),
            pending_cmds: Vec::new(),
        }
    }

    /// Execute one Cmd. Sends signal frames on sockets,
    /// schedules timers, etc. Skeleton — concrete dispatchers
    /// land as connection drivers wire up.
    pub fn execute_cmd(&mut self, _cmd: Cmd) -> crate::error::Result<()> {
        // Drivers not yet wired — Cmds queue but don't fire.
        Ok(())
    }
}

impl eframe::App for MentciEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Drain engine events. (Driver wiring lands later;
        //    for now, no events arrive.)

        // 2. Derive the view snapshot.
        let view = self.workbench.view();

        // 3. Paint, capturing user gestures.
        let mut user_events: Vec<UserEvent> = Vec::new();
        crate::render::workbench(ctx, &view, &mut user_events);

        // 4. Apply each captured event to the model.
        for ev in user_events {
            let cmds = self.workbench.on_user_event(ev);
            self.pending_cmds.extend(cmds);
        }

        // 5. Dispatch pending cmds.
        let cmds = std::mem::take(&mut self.pending_cmds);
        for cmd in cmds {
            // Errors surface as diagnostics in a future
            // iteration; for now, log to stderr.
            if let Err(e) = self.execute_cmd(cmd) {
                eprintln!("cmd dispatch failed: {e}");
            }
        }
    }
}
