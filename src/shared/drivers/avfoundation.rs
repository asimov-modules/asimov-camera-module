// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver};
use alloc::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct AvFoundationCameraDriver {
    pub config: CameraConfig,
}

impl dogma::Named for AvFoundationCameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "avfoundation".into()
    }
}

impl CameraDriver for AvFoundationCameraDriver {}
