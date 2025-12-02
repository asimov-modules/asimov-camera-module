// This is free and unencumbered software released into the public domain.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Bgra8,
    // Rgb8,
    // Yuv420,
    // ...
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub pixel_format: PixelFormat,
}

impl Frame {
    pub fn new_bgra(data: Vec<u8>, width: usize, height: usize, stride: usize) -> Self {
        Self {
            data,
            width,
            height,
            stride,
            pixel_format: PixelFormat::Bgra8,
        }
    }
}
