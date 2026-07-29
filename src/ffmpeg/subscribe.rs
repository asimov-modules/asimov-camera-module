// This is free and unencumbered software released into the public domain.

/// Mirrors `nativecam::SubscribeOptions`'s shape.
#[derive(Clone, Debug)]
pub struct SubscribeOptions {
    pub capacity: usize,
    pub throttle_fps: Option<f64>,
}

impl Default for SubscribeOptions {
    fn default() -> Self {
        Self {
            capacity: 2,
            throttle_fps: None,
        }
    }
}

impl SubscribeOptions {
    #[inline]
    pub fn with_throttle_fps(mut self, fps: f64) -> Self {
        self.throttle_fps = fps.is_finite().then_some(fps).filter(|f| *f > 0.0);
        self
    }
}
