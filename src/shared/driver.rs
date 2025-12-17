// This is free and unencumbered software released into the public domain.

use super::{CameraError, Frame};

/// Frame callback invoked by drivers when a new frame is available.
pub type FrameCallback = Box<dyn FnMut(Frame) + Send + 'static>;

/// Camera driver contract (callback-first, single sink).
///
/// The callback is provided at driver creation time (via `open_camera` / backend `open`).
pub trait CameraDriver {
    /// Start capturing frames.
    fn start(&mut self) -> Result<(), CameraError>;

    /// Stop capturing frames (should be idempotent).
    fn stop(&mut self) -> Result<(), CameraError> {
        Ok(())
    }
}
