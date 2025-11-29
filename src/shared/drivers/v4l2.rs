// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver};

#[derive(Clone, Debug)]
pub struct V4l2CameraDriver {
    pub config: CameraConfig,
}

impl CameraDriver for V4l2CameraDriver {}
