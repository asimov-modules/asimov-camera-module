// This is free and unencumbered software released into the public domain.

#[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
use crate::api::AndroidPreviewTarget;

use crate::{CameraError, DeviceInfo, FrameRef};

use crossbeam_channel as ch;

#[derive(Clone, Debug)]
pub struct CameraConfig {
    pub device: Option<DeviceInfo>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub buffer_raw: usize,
    pub buffer_frames: usize,
    pub throttle_fps: Option<f64>,
    pub diagnostics: bool,
    pub frame_tx: Option<ch::Sender<FrameRef>>,

    #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
    pub android_preview: AndroidPreviewTarget,
}

impl CameraConfig {
    pub fn builder() -> CameraConfigBuilder {
        CameraConfigBuilder::new()
    }

    pub fn normalized(mut self) -> Self {
        self.width = self.width.max(1);
        self.height = self.height.max(1);

        self.fps = if self.fps.is_finite() && self.fps > 0.0 {
            self.fps
        } else {
            30.0
        };

        self.buffer_raw = self.buffer_raw.max(1);
        self.buffer_frames = self.buffer_frames.max(1);

        self.throttle_fps = self.throttle_fps.filter(|x| x.is_finite() && *x > 0.0);

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
        if self.buffer_frames == 0 {
            return Err(CameraError::invalid_config("buffer_frames must be >= 1"));
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CameraConfigBuilder {
    device: Option<DeviceInfo>,
    width: u32,
    height: u32,
    fps: f64,
    buffer_raw: usize,
    buffer_frames: usize,
    throttle_fps: Option<f64>,
    diagnostics: bool,
    frame_tx: Option<ch::Sender<FrameRef>>,

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
            buffer_frames: 1,
            throttle_fps: None,
            diagnostics: false,
            frame_tx: None,

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

    pub fn buffer_frames(mut self, n: usize) -> Self {
        self.buffer_frames = n;
        self
    }

    pub fn throttle_fps(mut self, fps: Option<f64>) -> Self {
        self.throttle_fps = fps;
        self
    }

    pub fn diagnostics(mut self, enabled: bool) -> Self {
        self.diagnostics = enabled;
        self
    }

    pub fn frame_tx(mut self, tx: ch::Sender<FrameRef>) -> Self {
        self.frame_tx = Some(tx);
        self
    }

    #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
    pub fn android_preview(mut self, target: AndroidPreviewTarget) -> Self {
        self.android_preview = Some(target);
        self
    }

    pub fn build(self) -> Result<CameraConfig, CameraError> {
        #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
        {
            let android_preview = self.android_preview.ok_or_else(|| {
                CameraError::invalid_config(
                    "android_preview is required when building with mobile-preview on Android",
                )
            })?;

            let cfg = CameraConfig {
                device: self.device,
                width: self.width,
                height: self.height,
                fps: self.fps,
                buffer_raw: self.buffer_raw,
                buffer_frames: self.buffer_frames,
                throttle_fps: self.throttle_fps,
                diagnostics: self.diagnostics,
                frame_tx: self.frame_tx,
                android_preview,
            };

            let cfg = cfg.normalized();
            cfg.validate()?;
            return Ok(cfg);
        }

        #[cfg(not(all(feature = "mobile-preview", feature = "android", target_os = "android")))]
        {
            let cfg = CameraConfig {
                device: self.device,
                width: self.width,
                height: self.height,
                fps: self.fps,
                buffer_raw: self.buffer_raw,
                buffer_frames: self.buffer_frames,
                throttle_fps: self.throttle_fps,
                diagnostics: self.diagnostics,
                frame_tx: self.frame_tx,
            };

            let cfg = cfg.normalized();
            cfg.validate()?;
            Ok(cfg)
        }
    }
}
