//! Diagnostics pane — chronological list of validation
//! outcomes that aren't `Ok`.

use mentci_lib::diagnostics::DiagnosticsView;
use mentci_lib::UserEvent;

pub fn diagnostics(
    _ui: &mut egui::Ui,
    _view: &DiagnosticsView,
    _out_events: &mut Vec<UserEvent>,
) {
    todo!()
}
