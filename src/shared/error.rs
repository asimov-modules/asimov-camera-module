// This is free and unencumbered software released into the public domain.

use std::error::Error as StdError;
use thiserror::Error;

/// Error type for camera operations.
///
/// This type is part of the public API and is intended to be stable and
/// user-facing across library and CLI usage.
#[derive(Debug, Error)]
pub enum CameraError {
    /// No backend is available for the current build/target configuration.
    #[error("no suitable camera backend available")]
    NoDriver,

    /// No camera device is available or could not be opened.
    #[error("no camera device available")]
    NoCamera,

    /// The driver exists but is not configured (e.g., callback not set).
    #[error("driver is not configured")]
    NotConfigured,

    /// Requested operation is not supported by the selected backend/device.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Configuration is invalid (e.g., dimensions/fps/device selector).
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Backend failed due to an underlying driver/runtime error.
    #[error("driver error: {context}")]
    DriverError {
        context: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },

    /// Catch-all for unexpected failures; use sparingly.
    #[error("{0}")]
    Other(String),
}

impl CameraError {
    #[inline]
    pub fn driver(
        context: &'static str,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::DriverError {
            context,
            source: Box::new(source),
        }
    }

    #[inline]
    pub fn unsupported(msg: impl Into<String>) -> Self {
        Self::Unsupported(msg.into())
    }

    #[inline]
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        Self::InvalidConfig(msg.into())
    }

    #[inline]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
