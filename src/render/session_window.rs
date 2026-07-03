//! The live-session window and its host container.
//!
//! A [`SessionWindow`] pairs exactly one [`SessionCanvas`] with one
//! [`SessionStreamSubscription`]. Its state is fully self-contained — its own
//! canvas, its own subscription token — so several Mentci windows could exist
//! later (multi-monitor), each watching its own harness. Building more than one
//! window is DEFERRED; [`SessionViewApp`] hosts exactly one today, but nothing in
//! the shape forecloses more.

use std::sync::{Arc, Mutex};

use crate::render::canvas::SessionCanvas;
use crate::render::session_stream::SessionStreamSubscription;

/// One Mentci live-session window: a single canvas fed by a single push-based
/// subscription. The subscription is held until the runtime is available, then
/// handed to a pump task on first paint.
pub struct SessionWindow {
    title: String,
    canvas: Arc<Mutex<SessionCanvas>>,
    subscription: Option<SessionStreamSubscription>,
}

impl SessionWindow {
    pub fn new(title: impl Into<String>, subscription: SessionStreamSubscription) -> Self {
        Self {
            title: title.into(),
            canvas: Arc::new(Mutex::new(SessionCanvas::new())),
            subscription: Some(subscription),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// A shared handle to this window's canvas, so an external driver (a test, or
    /// a producer harness) can observe what the window renders.
    pub fn canvas(&self) -> Arc<Mutex<SessionCanvas>> {
        Arc::clone(&self.canvas)
    }

    /// Hand the subscription's pump to the runtime. The pump parks on
    /// `recv().await` and repaints on each push; the window never polls. Idempotent
    /// — the subscription is taken once.
    fn start_pump(&mut self, runtime: &tokio::runtime::Runtime, ctx: &egui::Context) {
        if let Some(subscription) = self.subscription.take() {
            runtime.spawn(subscription.run(self.canvas(), ctx.clone()));
        }
    }

    fn paint(&self, ui: &mut egui::Ui) {
        let canvas = self.canvas.lock().expect("session canvas lock");
        canvas.paint(ui);
    }
}

/// Hosts the set of live-session windows in one eframe app. One window is built
/// today; the `Vec` keeps the door open for multi-monitor Mentci windows without
/// building them.
pub struct SessionViewApp {
    runtime: tokio::runtime::Runtime,
    windows: Vec<SessionWindow>,
    pumps_started: bool,
}

impl SessionViewApp {
    pub fn new(runtime: tokio::runtime::Runtime, windows: Vec<SessionWindow>) -> Self {
        Self {
            runtime,
            windows,
            pumps_started: false,
        }
    }

    /// Build a single-window app — the one shape built today.
    pub fn single(runtime: tokio::runtime::Runtime, window: SessionWindow) -> Self {
        Self::new(runtime, vec![window])
    }

    fn start_pumps_if_needed(&mut self, ctx: &egui::Context) {
        if self.pumps_started {
            return;
        }
        self.pumps_started = true;
        for window in &mut self.windows {
            window.start_pump(&self.runtime, ctx);
        }
    }
}

impl eframe::App for SessionViewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.start_pumps_if_needed(ctx);

        egui::CentralPanel::default().show(ctx, |ui| match self.windows.as_slice() {
            [] => {
                ui.label("no session window configured");
            }
            [window] => window.paint(ui),
            windows => {
                // Multi-window is DEFERRED. Until real per-monitor viewports
                // exist, stack the panes so no configured window is silently
                // dropped.
                for window in windows {
                    ui.heading(window.title());
                    window.paint(ui);
                    ui.separator();
                }
            }
        });

        // No `request_repaint_after` here: repaints are PUSH-driven. Each
        // subscription pump calls `ctx.request_repaint()` when the producer
        // pushes; with nothing pushed, the UI stays quiet.
    }
}
