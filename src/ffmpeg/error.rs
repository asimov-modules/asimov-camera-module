// This is free and unencumbered software released into the public domain.

use std::{error::Error as StdError, sync::Arc};

/// Mirrors `nativecam::CameraError`'s shape (same variant names) so the CLI
/// can be written once against whichever of the two is in scope. This is a
/// standalone, unrelated type — not shared code — so this module can be
/// deleted outright without touching anything on the `nativecam` side.
pub type DynError = Arc<dyn StdError + Send + Sync + 'static>;
pub type Result<T> = core::result::Result<T, CameraError>;

#[derive(Clone, Debug, thiserror::Error)]
pub enum CameraError {
    /// Never actually constructed here (there's no multi-driver selection
    /// in this module) — kept only so the CLI's shared error-handling code
    /// can match on it regardless of which backend is in scope.
    #[error("no camera driver available")]
    #[allow(dead_code)]
    NoDriver,

    #[error("no camera device available")]
    NoCamera,

    #[error("requested camera device not found: {0}")]
    DeviceNotFound(String),

    #[error("camera not configured")]
    NotConfigured,

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("camera is closed")]
    Closed,

    #[error("{context}: {source}")]
    Driver {
        context: &'static str,
        #[source]
        source: DynError,
    },

    #[error("{0}")]
    Other(String),
}

impl CameraError {
    #[inline]
    pub fn driver(context: &'static str, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Driver {
            context,
            source: Arc::new(source),
        }
    }

    #[inline]
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        Self::InvalidConfig(msg.into())
    }

    #[inline]
    pub fn device_not_found(msg: impl Into<String>) -> Self {
        Self::DeviceNotFound(msg.into())
    }

    #[inline]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
