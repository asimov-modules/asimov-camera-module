// This is free and unencumbered software released into the public domain.

#[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
use crate::api::AndroidPreviewTarget;

use crate::{CameraError, DeviceInfo};

/// Capture parameters shared by the public `CameraConfig` (device optional,
/// resolved later) and the internal `DriverConfig` (device mandatory) — kept
/// as one struct so the two don't drift out of sync with each other.
#[derive(Clone, Debug)]
pub struct CaptureSettings {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub buffer_raw: usize,
    pub diagnostics: bool,

    #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
    pub android_preview: AndroidPreviewTarget,
}

impl CaptureSettings {
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

#[derive(Clone, Debug)]
pub struct CameraConfig {
    pub device: Option<DeviceInfo>,
    pub settings: CaptureSettings,
}

impl CameraConfig {
    pub fn builder() -> CameraConfigBuilder {
        CameraConfigBuilder::new()
    }

    pub fn normalized(mut self) -> Self {
        self.settings = self.settings.normalized();
        self
    }

    pub fn validate(&self) -> Result<(), CameraError> {
        self.settings.validate()
    }
}

#[derive(Clone, Debug)]
pub struct CameraConfigBuilder {
    device: Option<DeviceInfo>,
    width: u32,
    height: u32,
    fps: f64,
    buffer_raw: usize,
    diagnostics: bool,

    #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
    android_preview: Option<AndroidPreviewTarget>,
}

impl CameraConfigBuilder {
    fn new() -> Self {
        Self {
            device: None,
            width: 1280,
            height: 720,
            fps: 30.0,
            buffer_raw: 2,
            diagnostics: false,

            #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
            android_preview: None,
        }
    }

    pub fn device(mut self, device: DeviceInfo) -> Self {
        self.device = Some(device);
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.height = height;
        self
    }

    pub fn fps(mut self, fps: f64) -> Self {
        self.fps = fps;
        self
    }

    pub fn buffer_raw(mut self, n: usize) -> Self {
        self.buffer_raw = n;
        self
    }

    pub fn diagnostics(mut self, enabled: bool) -> Self {
        self.diagnostics = enabled;
        self
    }

    #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
    pub fn android_preview(mut self, target: AndroidPreviewTarget) -> Self {
        self.android_preview = Some(target);
        self
    }

    pub fn build(self) -> Result<CameraConfig, CameraError> {
        #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
        let android_preview = self.android_preview.ok_or_else(|| {
            CameraError::invalid_config(
                "android_preview is required when building with mobile-preview on Android",
            )
        })?;

        let settings = CaptureSettings {
            width: self.width,
            height: self.height,
            fps: self.fps,
            buffer_raw: self.buffer_raw,
            diagnostics: self.diagnostics,

            #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
            android_preview,
        };

        let cfg = CameraConfig {
            device: self.device,
            settings,
        }
        .normalized();

        cfg.validate()?;
        Ok(cfg)
    }
}
