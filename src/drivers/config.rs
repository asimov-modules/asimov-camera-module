// This is free and unencumbered software released into the public domain.

use crate::{CameraError, DeviceInfo};

#[derive(Clone, Debug)]
pub struct DriverConfig {
    pub device: DeviceInfo,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub buffer_raw: usize,
    #[allow(dead_code)]
    pub diagnostics: bool,

    /// Android preview target (ANativeWindow*) is required when building with
    /// `mobile-preview` on Android.
    #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
    pub android_preview: crate::AndroidPreviewTarget,
}

impl DriverConfig {
    pub fn normalized(mut self) -> Self {
        self.width = self.width.max(1);
        self.height = self.height.max(1);

        self.fps = if self.fps.is_finite() && self.fps > 0.0 {
            self.fps
        } else {
            30.0
        };

        self.buffer_raw = self.buffer_raw.max(1);

        self
    }

    pub fn validate(&self) -> Result<(), CameraError> {
        let id = self.device.id().trim();
        if id.is_empty() {
            return Err(CameraError::invalid_config("driver device id is empty"));
        }
        if self.width == 0 || self.height == 0 {
            return Err(CameraError::invalid_config("width/height must be > 0"));
        }
        if !self.fps.is_finite() || self.fps <= 0.0 {
            return Err(CameraError::invalid_config("fps must be finite and > 0"));
        }
        if self.buffer_raw == 0 {
            return Err(CameraError::invalid_config("buffer_raw must be >= 1"));
        }

        #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
        {
            if self.android_preview.as_ptr().is_null() {
                return Err(CameraError::invalid_config(
                    "android_preview must be a non-null native window pointer",
                ));
            }
        }

        Ok(())
    }
}
