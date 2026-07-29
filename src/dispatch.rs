// This is free and unencumbered software released into the public domain.

use std::sync::Mutex;

use crossbeam_channel as ch;

use crate::converter;
use crate::runtime::sampler::FpsSampler;
use crate::{FrameRef, PixelFormat, RawFrameRef, SubscribeOptions};

struct RawSink {
    tx: ch::Sender<RawFrameRef>,
    rx_probe: ch::Receiver<RawFrameRef>,
    sampler: Option<FpsSampler>,
}

struct ConvertedSink {
    format: PixelFormat,
    tx: ch::Sender<FrameRef>,
    rx_probe: ch::Receiver<FrameRef>,
    sampler: Option<FpsSampler>,
}

/// Fans out raw driver frames to any number of subscribers.
///
/// Each subscriber declares, at subscribe time, either "raw passthrough" or a
/// desired packed pixel format. A frame is converted to a given format at
/// most once per dispatch, regardless of how many subscribers requested that
/// same format.
#[derive(Default)]
pub struct FrameDistributor {
    raw: Mutex<Vec<RawSink>>,
    converted: Mutex<Vec<ConvertedSink>>,
}

impl FrameDistributor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe_raw(&self, opts: SubscribeOptions) -> ch::Receiver<RawFrameRef> {
        let (tx, rx) = ch::bounded(opts.capacity.max(1));
        let sink = RawSink {
            tx,
            rx_probe: rx.clone(),
            sampler: opts.throttle_fps.map(FpsSampler::new),
        };
        self.raw
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(sink);
        rx
    }

    pub fn subscribe_converted(
        &self,
        format: PixelFormat,
        opts: SubscribeOptions,
    ) -> ch::Receiver<FrameRef> {
        let (tx, rx) = ch::bounded(opts.capacity.max(1));
        let sink = ConvertedSink {
            format,
            tx,
            rx_probe: rx.clone(),
            sampler: opts.throttle_fps.map(FpsSampler::new),
        };
        self.converted
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(sink);
        rx
    }

    /// Dispatch one raw frame to every live subscriber, pruning any whose
    /// receiver has been dropped.
    pub fn dispatch(&self, raw: RawFrameRef) {
        {
            let mut sinks = self.raw.lock().unwrap_or_else(|p| p.into_inner());
            sinks.retain_mut(|sink| {
                if let Some(s) = sink.sampler.as_mut()
                    && !s.should_emit()
                {
                    return true;
                }
                send_latest(&sink.tx, &sink.rx_probe, raw.clone())
            });
        }

        let mut sinks = self.converted.lock().unwrap_or_else(|p| p.into_inner());
        if sinks.is_empty() {
            return;
        }

        // Convert each distinct requested format at most once per frame.
        let mut cache: Vec<(PixelFormat, Option<FrameRef>)> = Vec::new();

        sinks.retain_mut(|sink| {
            if let Some(s) = sink.sampler.as_mut()
                && !s.should_emit()
            {
                return true;
            }

            let converted = match cache.iter().find(|(fmt, _)| *fmt == sink.format) {
                Some((_, frame)) => frame.clone(),
                None => {
                    let frame = converter::convert_raw_to_frame(raw.clone(), sink.format);
                    cache.push((sink.format, frame.clone()));
                    frame
                },
            };

            // If this raw frame can't be converted to the requested format,
            // keep the subscriber registered and just skip this one frame.
            let Some(frame) = converted else {
                return true;
            };

            send_latest(&sink.tx, &sink.rx_probe, frame)
        });
    }
}

/// Send the latest frame, evicting a stale queued one on backpressure instead
/// of blocking or dropping the newest frame. Returns `false` when the
/// subscriber has disconnected (the caller should prune it).
fn send_latest<T: Clone>(tx: &ch::Sender<T>, rx_probe: &ch::Receiver<T>, item: T) -> bool {
    match tx.try_send(item.clone()) {
        Ok(()) => true,
        Err(ch::TrySendError::Disconnected(_)) => false,
        Err(ch::TrySendError::Full(_)) => {
            let _ = rx_probe.try_recv();
            !matches!(tx.try_send(item), Err(ch::TrySendError::Disconnected(_)))
        },
    }
}
