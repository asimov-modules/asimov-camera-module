// This is free and unencumbered software released into the public domain.

#![allow(dead_code)]

use core::ptr::null_mut;
use ndk_sys::{
    AImage, AImage_delete, AImage_getFormat, AImage_getHeight, AImage_getPlaneData,
    AImage_getPlanePixelStride, AImage_getPlaneRowStride, AImage_getTimestamp, AImage_getWidth,
    media_status_t,
};

use super::MediaResult;

#[derive(Debug, Default)]
pub struct Image {
    pub handle: *mut AImage,
}

impl Image {
    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }

    pub fn get_timestamp(&self) -> MediaResult<i64> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }

        let mut result: i64 = 0;
        let status = unsafe { AImage_getTimestamp(self.handle, &mut result) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result)
    }

    pub fn get_width(&self) -> MediaResult<u32> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }
        let mut v: i32 = 0;
        let st = unsafe { AImage_getWidth(self.handle, &mut v) };
        if st != media_status_t::AMEDIA_OK {
            return Err(st.into());
        }
        Ok(v.max(0) as u32)
    }

    pub fn get_height(&self) -> MediaResult<u32> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }
        let mut v: i32 = 0;
        let st = unsafe { AImage_getHeight(self.handle, &mut v) };
        if st != media_status_t::AMEDIA_OK {
            return Err(st.into());
        }
        Ok(v.max(0) as u32)
    }

    pub fn get_format(&self) -> MediaResult<i32> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }
        let mut v: i32 = 0;
        let st = unsafe { AImage_getFormat(self.handle, &mut v) };
        if st != media_status_t::AMEDIA_OK {
            return Err(st.into());
        }
        Ok(v)
    }

    pub fn get_plane_row_stride(&self, plane: i32) -> MediaResult<i32> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }
        let mut v: i32 = 0;
        let st = unsafe { AImage_getPlaneRowStride(self.handle, plane, &mut v) };
        if st != media_status_t::AMEDIA_OK {
            return Err(st.into());
        }
        Ok(v)
    }

    pub fn get_plane_pixel_stride(&self, plane: i32) -> MediaResult<i32> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }
        let mut v: i32 = 0;
        let st = unsafe { AImage_getPlanePixelStride(self.handle, plane, &mut v) };
        if st != media_status_t::AMEDIA_OK {
            return Err(st.into());
        }
        Ok(v)
    }

    pub fn copy_plane_data(&self, plane: i32) -> MediaResult<Vec<u8>> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }

        let mut data_ptr: *mut u8 = core::ptr::null_mut();
        let mut len: i32 = 0;

        let st = unsafe { AImage_getPlaneData(self.handle, plane, &mut data_ptr, &mut len) };
        if st != media_status_t::AMEDIA_OK {
            return Err(st.into());
        }
        if data_ptr.is_null() || len <= 0 {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }

        let n = len as usize;
        let mut out = Vec::with_capacity(n);
        unsafe {
            out.set_len(n);
            core::ptr::copy_nonoverlapping(data_ptr as *const u8, out.as_mut_ptr(), n);
        }
        Ok(out)
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                AImage_delete(self.handle);
                self.handle = null_mut();
            }
        }
    }
}
