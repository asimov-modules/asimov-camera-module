// This is free and unencumbered software released into the public domain.

use bytes::Bytes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb8,
    Bgra8,
}

impl PixelFormat {
    #[inline]
    pub const fn bytes_per_pixel(self) -> u32 {
        match self {
            PixelFormat::Rgb8 => 3,
            PixelFormat::Bgra8 => 4,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub data: Bytes,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub pixel_format: PixelFormat,
    pub timestamp_ns: u64,
}

impl Frame {
    #[inline]
    pub fn new(
        data: Bytes,
        width: u32,
        height: u32,
        stride: u32,
        pixel_format: PixelFormat,
    ) -> Self {
        Self {
            data,
            width,
            height,
            stride,
            pixel_format,
            timestamp_ns: 0,
        }
    }

    #[inline]
    pub fn new_rgb8(data: Bytes, width: u32, height: u32, stride: u32) -> Self {
        Self::new(data, width, height, stride, PixelFormat::Rgb8)
    }

    #[inline]
    pub fn new_bgra8(data: Bytes, width: u32, height: u32, stride: u32) -> Self {
        Self::new(data, width, height, stride, PixelFormat::Bgra8)
    }

    #[inline]
    pub fn with_timestamp_ns(mut self, timestamp_ns: u64) -> Self {
        self.timestamp_ns = timestamp_ns;
        self
    }

    #[inline]
    pub fn validate(&self) -> bool {
        let bpp = self.pixel_format.bytes_per_pixel();
        if self.width == 0 || self.height == 0 || self.stride == 0 {
            return false;
        }
        if self.stride < self.width.saturating_mul(bpp) {
            return false;
        }
        let min_len = (self.stride as usize).saturating_mul(self.height as usize);
        self.data.len() >= min_len
    }
}
