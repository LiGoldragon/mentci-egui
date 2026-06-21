//! mentci-egui — entry point.
//!
//! Constructs a tokio runtime for off-thread daemon calls, builds the
//! [`MentciEguiApp`], and opens the eframe window.

mod app;
mod daemon_client;
mod error;

use app::MentciEguiApp;

fn main() -> eframe::Result<()> {
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let native_options = eframe::NativeOptions {
        // Follow the OS light/dark preference; light is the fallback default
        // when the desktop reports no preference. The app also re-syncs the
        // system theme each frame through `dark-light` (see app.rs).
        follow_system_theme: true,
        default_theme: eframe::Theme::Light,
        viewport: egui::ViewportBuilder::default()
            .with_title("mentci")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "mentci",
        native_options,
        Box::new(move |_cc| Box::new(MentciEguiApp::new(tokio_runtime))),
    )
}
