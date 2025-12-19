// This is free and unencumbered software released into the public domain.

use std::error::Error as StdError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CameraError {
    #[error("no suitable camera backend available")]
    NoDriver,

    #[error("driver not applicable for this target/configuration")]
    NotApplicable,

    #[error("no camera device available")]
    NoCamera,

    #[error("driver is not configured")]
    NotConfigured,

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("stream closed")]
    Closed,

    #[error("driver error while {context}")]
    DriverError {
        context: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },

    #[error("{0}")]
    Other(String),
}

impl CameraError {
    #[inline]
    pub fn driver(context: &'static str, source: impl StdError + Send + Sync + 'static) -> Self {
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
