// This is free and unencumbered software released into the public domain.

use crate::{CameraBackend, CameraError};

#[derive(Clone, Debug)]
pub enum CameraEvent {
    Started {
        backend: CameraBackend,
    },
    Stopped {
        backend: CameraBackend,
    },
    Closed {
        backend: CameraBackend,
    },

    RawDropped {
        backend: CameraBackend,
    },
    FrameDropped {
        backend: CameraBackend,
    },

    Warning {
        backend: CameraBackend,
        message: String,
    },

    Error {
        backend: CameraBackend,
        error: CameraError,
    },
}
