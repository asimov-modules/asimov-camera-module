// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver};
use alloc::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct DshowCameraDriver {
    pub config: CameraConfig,
}

impl dogma::Named for DshowCameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "dshow".into()
    }
}

impl DshowCameraDriver {
    pub fn open(
        _input_url: impl AsRef<str>,
        config: CameraConfig,
        _callback: FrameCallback,
    ) -> Result<Self, CameraError> {
        Err(CameraError::NoDriver)
    }
}

impl CameraDriver for DshowCameraDriver {}
