//! Wire pane — every signal frame seen on this connection,
//! at typed-variant level. User-toggled.

use mentci_lib::wire::WireView;
use mentci_lib::UserEvent;

pub fn wire(
    ui: &mut egui::Ui,
    view: &WireView,
    out_events: &mut Vec<UserEvent>,
) {
    ui.horizontal(|ui| {
        ui.heading("WIRE");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let pause_label = if view.paused { "resume" } else { "pause" };
            if ui.small_button(pause_label).clicked() {
                out_events.push(if view.paused {
                    UserEvent::ResumeWire
                } else {
                    UserEvent::PauseWire
                });
            }
        });
    });

    egui::ScrollArea::vertical().show(ui, |ui| {
        if view.frames.is_empty() {
            ui.label("(no traffic yet)");
        } else {
            for entry in &view.frames {
                ui.label(format!(
                    "{:?} {} · {}",
                    entry.direction, entry.timestamp_iso, entry.verb_summary
                ));
            }
        }
    });
}
