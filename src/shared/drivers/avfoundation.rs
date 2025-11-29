// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver};

#[derive(Clone, Debug)]
pub struct AvFoundationCameraDriver {
    pub config: CameraConfig,
}

impl CameraDriver for AvFoundationCameraDriver {}
