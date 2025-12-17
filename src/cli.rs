// This is free and unencumbered software released into the public domain.

//! CLI helpers (error reporting, verbosity handling).
//!
//! This module must compile even when the crate feature `cli` is disabled,
//! because the library is built in non-CLI configurations.

#[cfg(feature = "cli")]
use crate::shared::CameraError;

#[cfg(feature = "cli")]
use asimov_module::SysexitsError::{self, *};

#[cfg(feature = "cli")]
use clientele::StandardOptions;

#[cfg(feature = "cli")]
pub fn handle_error(err: &CameraError, flags: &StandardOptions) -> SysexitsError {
    #[cfg(feature = "tracing")]
    {
        use asimov_module::tracing::{debug, error};

        error!(target: "asimov_camera_module", %err, "camera command failed");

        if flags.debug || flags.verbose >= 2 {
            debug!(target: "asimov_camera_module", ?err, "detailed error");
        }
    }

    report_error(err, flags);
    map_error_to_sysexit(err)
}

#[cfg(feature = "cli")]
pub fn info_user(flags: &StandardOptions, msg: &str) {
    if flags.debug || flags.verbose >= 1 {
        eprintln!("INFO: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::info!(target: "asimov_camera_module", "{msg}");
}

#[cfg(feature = "cli")]
pub fn warn_user(flags: &StandardOptions, msg: &str) {
    if flags.debug || flags.verbose >= 1 {
        eprintln!("WARN: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::warn!(target: "asimov_camera_module", "{msg}");
}

#[cfg(feature = "cli")]
pub fn warn_user_with_error(flags: &StandardOptions, msg: &str, error: &dyn std::error::Error) {
    if flags.debug || flags.verbose >= 2 {
        eprintln!("WARN: {msg}: {error}");
    } else if flags.verbose >= 1 {
        eprintln!("WARN: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::warn!(target: "asimov_camera_module", error = %error, "{msg}");
}

#[cfg(feature = "cli")]
fn report_error(err: &CameraError, flags: &StandardOptions) {
    use std::error::Error as _;
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
fn map_error_to_sysexit(err: &CameraError) -> SysexitsError {
    match err {
        CameraError::NoDriver => EX_UNAVAILABLE,
        CameraError::NoCamera => EX_USAGE,
        CameraError::NotConfigured => EX_CONFIG,
        CameraError::InvalidConfig(_) => EX_USAGE,
        CameraError::Unsupported(_) => EX_UNAVAILABLE,
        CameraError::DriverError { .. } => EX_SOFTWARE,
        CameraError::Other(_) => EX_SOFTWARE,
    }
}

// When `cli` is disabled, keep the module linkable without exposing CLI-only types.
#[cfg(not(feature = "cli"))]
#[inline]
pub fn info_user(_msg: &str) {}

#[cfg(not(feature = "cli"))]
#[inline]
pub fn warn_user(_msg: &str) {}
