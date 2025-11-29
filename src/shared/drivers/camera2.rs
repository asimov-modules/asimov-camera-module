// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver};
use alloc::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct Camera2CameraDriver {
    pub config: CameraConfig,
}

impl dogma::Named for Camera2CameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "camera2".into()
    }
}

impl CameraDriver for Camera2CameraDriver {}
