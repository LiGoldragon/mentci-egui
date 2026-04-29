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
    ctx: &egui::Context,
    view: &WorkbenchView,
    out_events: &mut Vec<UserEvent>,
) {
    // Header (top, always).
    egui::TopBottomPanel::top("header").show(ctx, |ui| {
        crate::render::header::header(ui, &view.header, out_events);
    });

    // Diagnostics strip (bottom, only when present).
    if let Some(diag) = &view.diagnostics {
        egui::TopBottomPanel::bottom("diagnostics")
            .resizable(true)
            .show(ctx, |ui| {
                crate::render::diagnostics::diagnostics(ui, diag, out_events);
            });
    }

    // Wire strip (bottom, only when toggled).
    if let Some(wire) = &view.wire {
        egui::TopBottomPanel::bottom("wire")
            .resizable(true)
            .show(ctx, |ui| {
                crate::render::wire::wire(ui, wire, out_events);
            });
    }

    // Graphs nav (left).
    egui::SidePanel::left("graphs_nav")
        .resizable(true)
        .default_width(180.0)
        .show(ctx, |ui| {
            ui.heading("graphs");
            ui.separator();
            if view.graphs_nav.graphs.is_empty() {
                ui.label("(no graphs yet)");
            } else {
                for entry in &view.graphs_nav.graphs {
                    ui.label(&entry.display_name);
                }
            }
        });

    // Inspector (right).
    egui::SidePanel::right("inspector")
        .resizable(true)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("inspector");
            ui.separator();
            crate::render::inspector::inspector(ui, &view.inspector, out_events);
        });

    // Canvas (centre).
    egui::CentralPanel::default().show(ctx, |ui| {
        crate::render::canvas::canvas(ui, &view.canvas, out_events);
    });

    // Constructor flow (modal overlay).
    if let Some(ctor) = &view.constructor {
        crate::render::constructor::constructor(ctx, ctor, out_events);
    }
}
