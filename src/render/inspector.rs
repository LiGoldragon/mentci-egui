//! Inspector pane — selected slot detail + history.

use mentci_lib::inspector::InspectorView;
use mentci_lib::UserEvent;

pub fn inspector(
    ui: &mut egui::Ui,
    view: &InspectorView,
    _out_events: &mut Vec<UserEvent>,
) {
    match &view.focused {
        None => {
            ui.label("(no selection)");
        }
        Some(focused) => {
            ui.label(format!("kind: {}", focused.kind));
            ui.label(format!("name: {}", focused.display_name));
            // History + as-nexus paint in a later iteration.
        }
    }

    if !view.pinned.is_empty() {
        ui.separator();
        ui.label("pinned:");
        for p in &view.pinned {
            ui.label(format!("• {}", p.display_name));
        }
    }
}
