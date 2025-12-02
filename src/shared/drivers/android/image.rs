// This is free and unencumbered software released into the public domain.

use super::MediaResult;
use core::ptr::null_mut;
use ndk_sys::{AImage, AImage_delete, AImage_getTimestamp, media_status_t};

#[derive(Debug, Default)]
pub struct Image {
    pub(crate) handle: *mut AImage,
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe { AImage_delete(self.handle) }
        self.handle = null_mut();
    }
}

impl Image {
    pub fn get_timestamp(&self) -> MediaResult<i64> {
        let mut result = 0;
        let status = unsafe { AImage_getTimestamp(self.handle, &mut result) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result as _)
    }
}
