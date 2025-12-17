// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver};
use alloc::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct V4l2CameraDriver {
    pub config: CameraConfig,
}

impl dogma::Named for V4l2CameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "v4l2".into()
    }
}

impl V4l2CameraDriver {
    pub fn open(
        _input_url: impl AsRef<str>,
        config: CameraConfig,
        _callback: FrameCallback,
    ) -> Result<Self, CameraError> {
        Err(CameraError::NoDriver)
    }
}

impl CameraDriver for V4l2CameraDriver {}
