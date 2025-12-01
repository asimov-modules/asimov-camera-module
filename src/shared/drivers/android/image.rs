// This is free and unencumbered software released into the public domain.

use crate::shared::CameraError;
use core::ptr::null_mut;
use ndk_sys::{AImage, AImage_delete, AImage_getTimestamp, media_status_t};

#[derive(Debug, Default)]
pub struct AndroidImage {
    pub(crate) handle: *mut AImage,
}

impl Drop for AndroidImage {
    fn drop(&mut self) {
        unsafe {
            AImage_delete(self.handle);
            self.handle = null_mut();
        }
    }
}

impl AndroidImage {
    pub fn get_timestamp(&self) -> Result<i64, CameraError> {
        let mut result = 0;
        let status;
        unsafe {
            status = AImage_getTimestamp(self.handle, &mut result);
        }
        if status != media_status_t::AMEDIA_OK {
            assert!(status != media_status_t::AMEDIA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::Other); // TODO
        }
        Ok(result as _)
    }
}
