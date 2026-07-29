// This is free and unencumbered software released into the public domain.

/// Options for a single `Camera::subscribe`/`subscribe_raw` call.
#[derive(Clone, Debug)]
pub struct SubscribeOptions {
    /// Bounded channel capacity for this subscriber. Once full, the oldest
    /// queued frame is dropped in favor of the newest (subscribers always
    /// see the latest frame rather than stalling on backpressure).
    pub capacity: usize,

    /// Optional independent output-rate cap for this subscriber, in frames
    /// per second. Capture itself is never throttled — only what this
    /// specific subscriber receives. `None` means "as fast as frames arrive."
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
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity.max(1);
        self
    }

    pub fn with_throttle_fps(mut self, fps: f64) -> Self {
        self.throttle_fps = fps.is_finite().then_some(fps).filter(|f| *f > 0.0);
        self
    }
}
