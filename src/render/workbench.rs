//! Top-level workbench layout.
//!
//! Composes the multi-pane shell:
//!
//! - Header (top): daemon connection states + toggles.
//! - Graphs nav (left, always visible).
//! - Canvas (centre, always visible) — kind-driven render.
//! - Inspector (right, always visible).
//! - Diagnostics (bottom strip; only when ≥1 unread).
//! - Wire (bottom strip; only when toggled on).
//! - Constructor flow (modal overlay; only when active).

use mentci_lib::{UserEvent, WorkbenchView};

/// Paint the workbench. Captured user events accumulate in
/// `out_events`.
pub fn workbench(
    _ctx: &egui::Context,
    _view: &WorkbenchView,
    _out_events: &mut Vec<UserEvent>,
) {
    todo!()
}
