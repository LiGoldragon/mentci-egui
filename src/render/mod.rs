//! Live-session rendering for the mentci-egui client.
//!
//! `mentci-egui` is the VIEW. This module renders a Claude session's turns as
//! the harness PUSHES them on `HarnessTranscriptStream`; it never polls a
//! provider and never re-derives session facts. The split is deliberate:
//!
//! - [`canvas::SessionCanvas`] is the pane. It owns view state and paints it,
//!   and depends only on the `signal-harness` contract vocabulary — never on the
//!   harness runtime. Provider and mapping logic do not re-enter it.
//! - [`session_stream::SessionStreamSubscription`] is the consumer subscriber.
//!   It opens ONE subscription against the harness producer plane, takes the
//!   connect snapshot, then parks on `recv().await` and folds each pushed
//!   delivery into a canvas. No poll loop, no interval tick.
//! - [`session_window::SessionWindow`] pairs one canvas with one subscription;
//!   [`session_window::SessionViewApp`] hosts a set of them. Exactly one window
//!   is built today, but the state is per-window so multi-monitor Mentci windows
//!   are not foreclosed (that build is deferred).

pub mod canvas;
pub mod session_stream;
pub mod session_window;

pub use canvas::{ObservedTurn, SessionCanvas, SessionConnection, SessionDelivery, TurnField};
pub use session_stream::SessionStreamSubscription;
pub use session_window::{SessionViewApp, SessionWindow};
