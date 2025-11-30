// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver, CameraError};
use alloc::borrow::Cow;
use ndk_sys::android_get_device_api_level;

#[derive(Clone, Debug, Default)]
pub struct Camera2CameraDriver {
    pub config: CameraConfig,
    pub api_level: u32,
}

impl Camera2CameraDriver {
    pub fn open(_input_url: impl AsRef<str>, config: CameraConfig) -> Result<Self, CameraError> {
        unsafe {
            let api_level = android_get_device_api_level() as u32;
            eprintln!("android_get_device_api_level={}", api_level);
            Ok(Camera2CameraDriver { config, api_level })
        }
    }
}

impl dogma::Named for Camera2CameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "camera2".into()
    }
}

impl CameraDriver for Camera2CameraDriver {}
