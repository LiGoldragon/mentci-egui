//! Header — daemon connection states + global toggles.
//!
//! Shows both criome and nexus-daemon connection status
//! explicitly. Auto-reconnect is rejected by design — the user
//! sees why a daemon disconnected and reconnects deliberately.

use mentci_lib::connection::{ConnectionView, DaemonStatus};
use mentci_lib::view::HeaderView;
use mentci_lib::UserEvent;

pub fn header(
    ui: &mut egui::Ui,
    view: &HeaderView,
    out_events: &mut Vec<UserEvent>,
) {
    ui.horizontal(|ui| {
        connection_chip(ui, &view.criome, out_events, ChipDaemon::Criome);
        ui.separator();
        connection_chip(ui, &view.nexus, out_events, ChipDaemon::Nexus);

        ui.add_space(20.0);

        // Right-aligned toggles.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .selectable_label(view.tweaks_open, "⌗ tweaks")
                .clicked()
            {
                out_events.push(UserEvent::ToggleTweaksPane);
            }
            if ui
                .selectable_label(view.wire_toggled_on, "⊞ wire")
                .clicked()
            {
                out_events.push(UserEvent::ToggleWirePane);
            }
        });
    });
}

#[derive(Copy, Clone)]
enum ChipDaemon {
    Criome,
    Nexus,
}

fn connection_chip(
    ui: &mut egui::Ui,
    cv: &ConnectionView,
    out_events: &mut Vec<UserEvent>,
    daemon: ChipDaemon,
) {
    let glyph = match cv.status {
        DaemonStatus::Disconnected => "○",
        DaemonStatus::Connecting => "◐",
        DaemonStatus::Handshaking => "◑",
        DaemonStatus::Connected => "●",
    };
    let status_label = match cv.status {
        DaemonStatus::Disconnected => "disconnected".to_string(),
        DaemonStatus::Connecting => "connecting…".to_string(),
        DaemonStatus::Handshaking => "handshaking".to_string(),
        DaemonStatus::Connected => match &cv.version {
            Some(v) => format!("connected · v{v}"),
            None => "connected".to_string(),
        },
    };

    let chip_text = format!("{glyph} {} · {}", cv.label, status_label);

    let response = ui.label(chip_text);
    if let Some(note) = &cv.note {
        response.on_hover_text(note);
    }

    if matches!(cv.status, DaemonStatus::Disconnected)
        && ui.small_button("reconnect").clicked()
    {
        out_events.push(match daemon {
            ChipDaemon::Criome => UserEvent::ReconnectCriome,
            ChipDaemon::Nexus => UserEvent::ReconnectNexus,
        });
    }
}
