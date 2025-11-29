// This is free and unencumbered software released into the public domain.

use super::CameraError;
use core::result::Result;

pub trait CameraDriver {
    fn start(&mut self) -> Result<(), CameraError> {
        Err(CameraError::NoCamera)
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        Ok(())
    }
}
