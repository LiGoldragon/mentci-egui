//! Render dispatcher.
//!
//! Each pane has its own paint function. The top-level
//! [`workbench`] composes them according to the
//! `WorkbenchView` snapshot. Per-pane functions take the view
//! data and a `&mut Vec<UserEvent>` to push captured events
//! into.

pub mod canvas;
pub mod constructor;
pub mod diagnostics;
pub mod header;
pub mod inspector;
pub mod wire;
pub mod workbench;

pub use workbench::workbench;
