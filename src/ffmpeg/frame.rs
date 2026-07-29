// This is free and unencumbered software released into the public domain.

use bytes::Bytes;
use std::sync::Arc;

/// Mirrors `nativecam::PixelFormat`'s shape. ffmpeg only ever emits `Rgb8`
/// here (that's what's requested from the `ffmpeg` process itself), so
/// `Bgra8` exists only so CLI code written against either backend compiles;
/// requesting it from `Camera::subscribe` fails with `Unsupported`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb8,
    Bgra8,
}

/// Mirrors `nativecam::Frame`'s shape.
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: PixelFormat,
    pub data: Bytes,
    pub timestamp_ns: Option<u64>,
}

pub type FrameRef = Arc<Frame>;
