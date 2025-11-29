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

impl CameraDriver for V4l2CameraDriver {}
