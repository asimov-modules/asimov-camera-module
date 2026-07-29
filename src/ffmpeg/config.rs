// This is free and unencumbered software released into the public domain.

use crate::ffmpeg::{CameraError, DeviceInfo, Result};

/// Mirrors `nativecam::CameraConfig`'s shape (flattened — no `CaptureSettings`
/// split, since there's no mobile-preview concept to keep separate here).
#[derive(Clone, Debug)]
pub struct CameraConfig {
    pub device: Option<DeviceInfo>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub diagnostics: bool,
}

impl CameraConfig {
    pub fn builder() -> CameraConfigBuilder {
        CameraConfigBuilder::new()
    }
}

#[derive(Clone, Debug)]
pub struct CameraConfigBuilder {
    device: Option<DeviceInfo>,
    width: u32,
    height: u32,
    fps: f64,
    diagnostics: bool,
}

impl CameraConfigBuilder {
    fn new() -> Self {
        Self {
            device: None,
            width: 1280,
            height: 720,
            fps: 30.0,
            diagnostics: false,
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

    pub fn diagnostics(mut self, enabled: bool) -> Self {
        self.diagnostics = enabled;
        self
    }

    pub fn build(self) -> Result<CameraConfig> {
        if self.width == 0 || self.height == 0 {
            return Err(CameraError::invalid_config("width/height must be > 0"));
        }
        if !self.fps.is_finite() || self.fps <= 0.0 {
            return Err(CameraError::invalid_config("fps must be finite and > 0"));
        }

        Ok(CameraConfig {
            device: self.device,
            width: self.width,
            height: self.height,
            fps: self.fps,
            diagnostics: self.diagnostics,
        })
    }
}
