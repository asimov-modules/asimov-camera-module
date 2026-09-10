// This is free and unencumbered software released into the public domain.

use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct FpsSampler {
    period: Duration,
    next_deadline: Instant,
}

impl FpsSampler {
    pub fn new(fps: f64) -> Self {
        let fps = if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            30.0
        };
        let period = Duration::from_secs_f64(1.0 / fps);
        let now = Instant::now();
        Self {
            period,
            next_deadline: now + period,
        }
    }

    /// Returns `true` if we should emit now (i.e., deadline reached), and advances the deadline.
    #[inline]
    pub fn should_emit(&mut self) -> bool {
        let now = Instant::now();
        if now >= self.next_deadline {
            self.advance_deadline();
            true
        } else {
            false
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn time_until_deadline(&self) -> Duration {
        let now = Instant::now();
        self.next_deadline.saturating_duration_since(now)
    }

    #[inline]
    pub fn advance_deadline(&mut self) {
        let now = Instant::now();
        if now >= self.next_deadline {
            self.next_deadline = now + self.period;
        } else {
            self.next_deadline += self.period;
        }
    }
}
