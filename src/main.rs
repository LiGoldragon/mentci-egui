//! mentci-egui — entry point.
//!
//! Constructs a tokio runtime (so connection drivers can run
//! UDS I/O off the egui thread), builds the [`MentciEguiApp`]
//! (which wraps a fresh [`mentci_lib::WorkbenchState`]), opens
//! the eframe window. Daemon connections are auto-attempted
//! on the first frame; the user sees the lifecycle on screen.

mod app;
mod error;
mod render;

use app::MentciEguiApp;

fn main() -> eframe::Result<()> {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

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
        Box::new(move |_cc| {
            Box::new(MentciEguiApp::new(default_principal(), tokio_runtime))
        }),
    )
}

/// Default Principal slot for the first session. Genesis seed
/// reserves slots `[0, 1024)`; the local human is slot 0 by
/// convention until multi-Principal lands.
fn default_principal() -> signal::Slot {
    signal::Slot::from(0u64)
}
