// This is free and unencumbered software released into the public domain.

/// Configuration for camera drivers.
#[derive(Clone, Debug, Default)]
pub struct CameraConfig {
    pub device: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

impl CameraConfig {
    pub fn new(device: String, width: u32, height: u32, fps: f64) -> Self {
        Self {
            device,
            width,
            height,
            fps,
        }
    }
}
