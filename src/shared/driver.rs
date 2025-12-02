// This is free and unencumbered software released into the public domain.

use super::{CameraError, Frame};
use core::result::Result;

pub type FrameCallback = Box<dyn Fn(Frame) + Send + Sync>;

pub trait CameraDriver {
    fn start(&mut self) -> Result<(), CameraError> {
        Err(CameraError::NoCamera)
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        Ok(())
    }
}
