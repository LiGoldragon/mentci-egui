//! mentci-egui — entry point.
//!
//! Constructs the [`MentciEguiApp`] (which wraps a fresh
//! [`mentci_lib::WorkbenchState`]), opens the eframe window,
//! runs the per-frame loop. Daemon connections open
//! asynchronously after the first frame paints — the user
//! sees disconnected status, then transitions as the
//! handshakes complete.

mod app;
mod error;
mod render;

use app::MentciEguiApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("mentci")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "mentci",
        native_options,
        Box::new(|_cc| Box::new(MentciEguiApp::new(default_principal()))),
    )
}

/// Default Principal slot for the first session. Genesis seed
/// reserves slots `[0, 1024)`; the local human is slot 0 by
/// convention until multi-Principal lands.
fn default_principal() -> signal::Slot {
    signal::Slot::from(0u64)
}
