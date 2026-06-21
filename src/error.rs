//! Typed Error enum for shell-level failures.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("eframe initialisation failed: {0}")]
    EframeInit(String),

    #[error("daemon IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("signal-mentci frame error: {0}")]
    SignalMentci(#[from] signal_mentci::SignalFrameError),

    #[error("signal frame error: {0}")]
    SignalFrame(#[from] signal_frame::FrameError),

    #[error("daemon frame too large: maximum {maximum} bytes, found {found}")]
    FrameTooLarge { maximum: usize, found: usize },

    #[error("unexpected daemon frame: {0}")]
    UnexpectedDaemonFrame(String),

    /// The introspect query (the universal-client second component) failed.
    /// Carries mentci-lib's typed introspect error so the shell surfaces the
    /// real cause rather than a String catch-all.
    #[error("introspect query failed: {0}")]
    Introspect(#[from] mentci_lib::Error),
}

pub type Result<T> = core::result::Result<T, Error>;
