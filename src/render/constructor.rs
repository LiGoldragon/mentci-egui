//! Constructor flow rendering — modal/in-place editors per
//! verb-flow.

use mentci_lib::constructor::ConstructorView;
use mentci_lib::UserEvent;

pub fn constructor(
    ctx: &egui::Context,
    _view: &ConstructorView,
    out_events: &mut Vec<UserEvent>,
) {
    egui::Window::new("constructor")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("(constructor flows land as gestures wire up)");
            if ui.button("cancel").clicked() {
                out_events.push(UserEvent::ConstructorCancel);
            }
        });
}
