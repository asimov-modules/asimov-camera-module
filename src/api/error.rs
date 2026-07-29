// This is free and unencumbered software released into the public domain.

use std::{error::Error as StdError, sync::Arc};

pub type DynError = Arc<dyn StdError + Send + Sync + 'static>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CameraErrorKind {
    NoDriver,
    NotApplicable,
    NoCamera,
    DeviceNotFound,
    DeviceBusy,
    NotConfigured,
    PermissionDenied,
    Unsupported,
    InvalidConfig,
    Closed,
    Driver,
    Other,
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum CameraError {
    #[error("no camera driver available")]
    NoDriver,

    #[error("driver not applicable")]
    NotApplicable,

    #[error("no camera device available")]
    NoCamera,

    /// A specific requested device (by id or name) does not exist among the
    /// currently enumerated devices — distinct from `NoCamera` (no camera
    /// hardware at all). Typically means the id is stale (device unplugged
    /// since it was listed) or was mistyped by the caller.
    #[error("requested camera device not found: {0}")]
    DeviceNotFound(String),

    /// The camera exists but is already held by another app/process (or the
    /// platform's concurrent-camera-session limit was hit). Not a bug in
    /// this module — the caller should retry later or tell the user to
    /// close whatever else is using the camera.
    #[error("camera device is busy: {0}")]
    DeviceBusy(String),

    #[error("camera not configured")]
    NotConfigured,

    /// The OS denied (or has not yet granted) camera access for this
    /// process. On iOS/macOS/Android this is a normal, expected outcome the
    /// first time an app requests the camera — callers should surface a
    /// permission prompt/settings link, not treat it as an internal error.
    #[error("camera permission denied")]
    PermissionDenied,

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
    pub const fn kind(&self) -> CameraErrorKind {
        match self {
            Self::NoDriver => CameraErrorKind::NoDriver,
            Self::NotApplicable => CameraErrorKind::NotApplicable,
            Self::NoCamera => CameraErrorKind::NoCamera,
            Self::DeviceNotFound(_) => CameraErrorKind::DeviceNotFound,
            Self::DeviceBusy(_) => CameraErrorKind::DeviceBusy,
            Self::NotConfigured => CameraErrorKind::NotConfigured,
            Self::PermissionDenied => CameraErrorKind::PermissionDenied,
            Self::Unsupported(_) => CameraErrorKind::Unsupported,
            Self::InvalidConfig(_) => CameraErrorKind::InvalidConfig,
            Self::Closed => CameraErrorKind::Closed,
            Self::Driver { .. } => CameraErrorKind::Driver,
            Self::Other(_) => CameraErrorKind::Other,
        }
    }

    #[inline]
    pub fn driver(context: &'static str, source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Driver {
            context,
            source: Arc::new(source),
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

    #[inline]
    pub fn device_not_found(msg: impl Into<String>) -> Self {
        Self::DeviceNotFound(msg.into())
    }

    #[inline]
    pub fn device_busy(msg: impl Into<String>) -> Self {
        Self::DeviceBusy(msg.into())
    }

    #[inline]
    pub const fn is_not_applicable(&self) -> bool {
        matches!(self, Self::NotApplicable)
    }

    #[inline]
    pub const fn is_expected(&self) -> bool {
        matches!(
            self,
            Self::NotApplicable
                | Self::NoDriver
                | Self::NoCamera
                | Self::Closed
                | Self::PermissionDenied
                | Self::DeviceBusy(_)
        )
    }
}

impl From<DynError> for CameraError {
    fn from(e: DynError) -> Self {
        CameraError::Driver {
            context: "error",
            source: e,
        }
    }
}
