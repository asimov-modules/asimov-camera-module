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

impl CameraDriver for DshowCameraDriver {}
