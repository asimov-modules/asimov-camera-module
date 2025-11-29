// This is free and unencumbered software released into the public domain.

use std::error::Error as StdError;
use thiserror::Error;

/// Result type used by this crate.
pub type Result<T = (), E = Error> = std::result::Result<T, E>;

/// Error type for operations in the camera module.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error while {context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("I/O error: {0}")]
    IoPlain(#[from] std::io::Error),

    #[error("ffmpeg invocation failed: {0}")]
    FfmpegSpawn(String),

    #[error("ffmpeg exited with non-zero status: {0}")]
    FfmpegExit(String),

    #[error("camera device not available: {0}")]
    CameraUnavailable(String),

    #[error("invalid frame size: {0}")]
    InvalidFrameSize(String),

    #[error("JSON-LD conversion failed: {0}")]
    JsonLd(String),

    /// Generic error with a human-readable message.
    #[error("{0}")]
    Other(String),
}

/// Helper to construct a boxed error from a string.
pub fn err_msg<M: Into<String>>(m: M) -> Box<dyn StdError> {
    m.into().into()
}

#[cfg(feature = "cli")]
use asimov_module::SysexitsError::{self, *};

#[cfg(feature = "cli")]
use clientele::StandardOptions;

/// Handle a fatal error: log, print and map to an appropriate `SysexitsError`.
#[cfg(feature = "cli")]
pub fn handle_error(err: &Error, flags: &StandardOptions) -> SysexitsError {
    #[cfg(feature = "tracing")]
    {
        use asimov_module::tracing::{debug, error};

        error!(target: "asimov_camera_module", %err, "camera command failed");

        if flags.debug || flags.verbose >= 2 {
            debug!(target: "asimov_camera_module", ?err, "detailed error");
        }
    }

    // Human-readable stderr output.
    report_error(err, flags);

    // Exit code.
    map_error_to_sysexit(err)
}

/// Log an informational message for the user and via tracing.
#[cfg(feature = "cli")]
pub fn info_user(flags: &StandardOptions, msg: &str) {
    if flags.debug || flags.verbose >= 1 {
        eprintln!("INFO: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::info!(target: "asimov_camera_module", "{msg}");
}

/// Log a warning message for the user and via tracing (no attached error).
#[cfg(feature = "cli")]
pub fn warn_user(flags: &StandardOptions, msg: &str) {
    if flags.debug || flags.verbose >= 1 {
        eprintln!("WARN: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::warn!(target: "asimov_camera_module", "{msg}");
}

/// Log a warning with error context for the user and via tracing.
#[cfg(feature = "cli")]
pub fn warn_user_with_error(flags: &StandardOptions, msg: &str, error: &dyn StdError) {
    if flags.debug || flags.verbose >= 2 {
        eprintln!("WARN: {msg}: {error}");
    } else if flags.verbose >= 1 {
        eprintln!("WARN: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::warn!(
        target: "asimov_camera_module",
        error = %error,
        "{msg}"
    );
}

#[cfg(feature = "cli")]
fn report_error(err: &Error, flags: &StandardOptions) {
    use std::io::Write;

    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "ERROR: {err}");

    if flags.debug || flags.verbose >= 2 {
        let mut source = err.source();
        while let Some(cause) = source {
            let _ = writeln!(stderr, "  Caused by: {}", cause);
            source = cause.source();
        }
    }
}

#[cfg(feature = "cli")]
fn map_error_to_sysexit(err: &Error) -> SysexitsError {
    match err {
        Error::Io { .. } | Error::IoPlain(_) => EX_IOERR,
        Error::CameraUnavailable(_) | Error::InvalidFrameSize(_) => EX_USAGE,
        Error::FfmpegSpawn(_) | Error::FfmpegExit(_) => EX_UNAVAILABLE,
        Error::JsonLd(_) | Error::Other(_) => EX_SOFTWARE,
    }
}
