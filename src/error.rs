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

    #[error("unexpected control frame: {0}")]
    UnexpectedControlFrame(String),

    #[error("control socket parse error: {0}")]
    ControlParse(String),

    #[error("control request channel failed: {0}")]
    ControlRequest(String),

    #[error("control reply channel failed: {0}")]
    ControlReply(String),
}

pub type Result<T> = core::result::Result<T, Error>;
