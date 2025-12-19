// This is free and unencumbered software released into the public domain.

use crate::shared::PixelFormat;

#[derive(Clone, Debug)]
pub struct CameraConfig {
    pub device: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub pixel_format: Option<PixelFormat>,
    pub buffer_frames: usize,
    pub diagnostics: bool,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            device: None,
            width: 640,
            height: 480,
            fps: 30.0,
            pixel_format: None,
            buffer_frames: 2,
            diagnostics: false,
        }
    }
}

impl CameraConfig {
    pub fn new(width: u32, height: u32, fps: f64) -> Self {
        Self {
            width,
            height,
            fps,
            ..Default::default()
        }
    }

    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = Some(device.into());
        self
    }

    pub fn with_pixel_format(mut self, fmt: PixelFormat) -> Self {
        self.pixel_format = Some(fmt);
        self
    }

    pub fn with_buffer_frames(mut self, n: usize) -> Self {
        self.buffer_frames = n.max(1);
        self
    }

    pub fn with_diagnostics(mut self, enabled: bool) -> Self {
        self.diagnostics = enabled;
        self
    }
}
