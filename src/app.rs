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

use mentci_lib::{Cmd, WorkbenchState};

pub struct MentciEguiApp {
    pub workbench: WorkbenchState,
    /// Pending Cmds to dispatch this frame.
    pub pending_cmds: Vec<Cmd>,
    // todo!() — connection drivers, frame buffers, etc.
}

impl MentciEguiApp {
    pub fn new(_principal: signal::Slot) -> Self {
        todo!()
    }

    /// Execute one Cmd. Sends signal frames on sockets,
    /// schedules timers, etc.
    pub fn execute_cmd(&mut self, _cmd: Cmd) -> crate::error::Result<()> {
        todo!()
    }
}

impl eframe::App for MentciEguiApp {
    fn update(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        todo!()
    }
}
