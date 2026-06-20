//! Typed Error enum for shell-level failures.
//!
//! mentci-lib has its own Error enum for application-level
//! failures; this enum is for shell-level failures (eframe
//! init, Cmd dispatch errors).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("eframe initialisation failed: {0}")]
    EframeInit(String),

    #[error("cmd dispatch failed: {0}")]
    CmdDispatch(String),

    #[error("library error: {0}")]
    Lib(#[from] mentci_lib::Error),

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
}

pub type Result<T> = core::result::Result<T, Error>;
