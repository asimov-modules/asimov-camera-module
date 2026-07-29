// This is free and unencumbered software released into the public domain.

use core::ffi::c_void;

use bytes::Bytes;
use ndk_sys as ndk;

use crate::{RawFormat, RawFrame, RawFrameRef, RawPlane};

use super::callbacks::State;
use crate::drivers::android::ndk::{ImageReader, NativeWindow, acquire_latest_image_from_raw};

use std::sync::Arc;

pub struct ImageStream {
    reader: ImageReader,
    window: NativeWindow,
}

impl ImageStream {
    pub fn new(
        dimensions: (u32, u32),
        format: i32,
        max_images: i32,
        state_ptr: *mut c_void,
    ) -> Result<Self, crate::drivers::android::ndk::MediaStatus> {
        let mut reader = ImageReader::new(dimensions, format, max_images)?;

        let mut window = reader.get_window()?;
        window.acquire();

        extern "C" fn on_image_available(ctx: *mut c_void, reader: *mut ndk::AImageReader) {
            if ctx.is_null() || reader.is_null() {
                return;
            }
            let st = unsafe { &*(ctx as *const State) };

            if st.raw_tx.is_full() {
                let _ = unsafe { acquire_latest_image_from_raw(reader) };
                return;
            }

            let img = match unsafe { acquire_latest_image_from_raw(reader) } {
                Ok(v) => v,
                Err(_) => return,
            };

            let width = match img.get_width() {
                Ok(v) => v,
                Err(_) => return,
            };
            let height = match img.get_height() {
                Ok(v) => v,
                Err(_) => return,
            };

            let ts_ns = img.get_timestamp().ok().map(|v| v.max(0) as u64);

            let mut planes = Vec::with_capacity(3);

            for plane_idx in 0..3 {
                let data = match img.copy_plane_data(plane_idx) {
                    Ok(v) => v,
                    Err(_) => return,
                };

                let row_stride =
                    img.get_plane_row_stride(plane_idx).ok().unwrap_or(0).max(0) as u32;
                let pixel_stride = img
                    .get_plane_pixel_stride(plane_idx)
                    .ok()
                    .unwrap_or(1)
                    .max(1) as u32;

                planes.push(RawPlane::new(Bytes::from(data), row_stride, pixel_stride));
            }

            let raw = RawFrame::new(width, height, RawFormat::Yuv420Triplanar, planes, ts_ns);
            let frame_ref: RawFrameRef = Arc::new(raw);

            let _ = st.raw_tx.try_send(frame_ref);
        }

        reader.set_image_listener(state_ptr, on_image_available)?;

        Ok(Self { reader, window })
    }

    #[inline]
    pub fn window_ptr(&self) -> *mut ndk::ANativeWindow {
        self.window.as_ptr()
    }

    pub fn close(&mut self) {
        self.reader.close();
        self.window = NativeWindow::default();
    }
}

impl Drop for ImageStream {
    fn drop(&mut self) {
        self.close();
    }
}
