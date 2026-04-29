//! Diagnostics pane — chronological list of validation
//! outcomes that aren't `Ok`.

use mentci_lib::diagnostics::DiagnosticsView;
use mentci_lib::UserEvent;

pub fn diagnostics(
    ui: &mut egui::Ui,
    view: &DiagnosticsView,
    out_events: &mut Vec<UserEvent>,
) {
    ui.horizontal(|ui| {
        ui.heading(format!("⚠ DIAGNOSTICS ({})", view.entries.len()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("clear").clicked() {
                out_events.push(UserEvent::ClearDiagnostics);
            }
        });
    });

    egui::ScrollArea::vertical().show(ui, |ui| {
        for entry in &view.entries {
            ui.label(format!(
                "{} {} · {}",
                severity_glyph(&entry.severity),
                entry.code,
                entry.message
            ));
        }
    });
}

fn severity_glyph(s: &mentci_lib::diagnostics::DiagnosticSeverity) -> &'static str {
    match s {
        mentci_lib::diagnostics::DiagnosticSeverity::Ok => "✓",
        mentci_lib::diagnostics::DiagnosticSeverity::Warning => "⚠",
        mentci_lib::diagnostics::DiagnosticSeverity::Error => "✗",
    }
}
