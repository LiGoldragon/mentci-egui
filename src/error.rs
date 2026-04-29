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
}

pub type Result<T> = core::result::Result<T, Error>;
