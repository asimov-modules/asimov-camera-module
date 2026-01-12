// This is free and unencumbered software released into the public domain.

use bytes::Bytes;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// Packed RGB, 8-bit per channel, 3 bytes per pixel.
    Rgb8,
    /// Packed BGRA, 8-bit per channel, 4 bytes per pixel.
    Bgra8,
}

impl PixelFormat {
    #[inline]
    pub const fn bytes_per_pixel_packed(self) -> Option<u32> {
        match self {
            PixelFormat::Rgb8 => Some(3),
            PixelFormat::Bgra8 => Some(4),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawFormat {
    AndroidYuv420888,
    PackedRgb8,
    PackedBgra8,
}

#[derive(Clone, Debug)]
pub struct RawPlane {
    pub data: Bytes,
    pub row_stride: u32,
    pub pixel_stride: u32,
}

impl RawPlane {
    #[inline]
    pub fn new(data: Bytes, row_stride: u32, pixel_stride: u32) -> Self {
        Self {
            data,
            row_stride,
            pixel_stride,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub format: RawFormat,
    pub planes: Vec<RawPlane>,
    pub timestamp_ns: Option<u64>,
}

impl RawFrame {
    #[inline]
    pub fn new(
        width: u32,
        height: u32,
        format: RawFormat,
        planes: Vec<RawPlane>,
        timestamp_ns: Option<u64>,
    ) -> Self {
        Self {
            width,
            height,
            format,
            planes,
            timestamp_ns,
        }
    }

    #[inline]
    pub fn new_rgb8(
        width: u32,
        height: u32,
        data: Vec<u8>,
        row_stride: u32,
        timestamp_ns: Option<u64>,
    ) -> Self {
        RawFrame::new(
            width,
            height,
            RawFormat::PackedRgb8,
            vec![RawPlane::new(Bytes::from(data), row_stride, 3)],
            timestamp_ns,
        )
    }

    #[inline]
    pub fn new_bgra8(
        width: u32,
        height: u32,
        data: Vec<u8>,
        row_stride: u32,
        timestamp_ns: Option<u64>,
    ) -> Self {
        RawFrame::new(
            width,
            height,
            RawFormat::PackedBgra8,
            vec![RawPlane::new(Bytes::from(data), row_stride, 4)],
            timestamp_ns,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,

    /// Bytes between successive rows for packed formats (Rgb8/Bgra8).
    pub stride: u32,

    pub pixel_format: PixelFormat,

    /// Packed pixel bytes. For `Rgb8`: len ~= stride*height. For `Bgra8`: same.
    pub data: Bytes,

    pub timestamp_ns: Option<u64>,
}

impl Frame {
    #[inline]
    pub fn new(
        width: u32,
        height: u32,
        stride: u32,
        pixel_format: PixelFormat,
        data: Bytes,
        timestamp_ns: Option<u64>,
    ) -> Self {
        Self {
            width,
            height,
            stride,
            pixel_format,
            data,
            timestamp_ns,
        }
    }

    #[inline]
    pub fn packed_len_expected(&self) -> Option<usize> {
        let bpp = self.pixel_format.bytes_per_pixel_packed()? as usize;
        let min_stride = (self.width as usize).saturating_mul(bpp);
        let stride = (self.stride as usize).max(min_stride);
        Some(stride.saturating_mul(self.height as usize))
    }
}

pub type RawFrameRef = Arc<RawFrame>;
pub type FrameRef = Arc<Frame>;
