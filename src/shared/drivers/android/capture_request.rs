// This is free and unencumbered software released into the public domain.

use core::ptr::null_mut;
use ndk_sys::{ACaptureRequest, ACaptureRequest_free};

#[derive(Debug, Default)]
pub struct AndroidCaptureRequest {
    pub(crate) handle: *mut ACaptureRequest,
}

impl Drop for AndroidCaptureRequest {
    fn drop(&mut self) {
        unsafe {
            ACaptureRequest_free(self.handle);
            self.handle = null_mut();
        }
    }
}

impl AndroidCaptureRequest {}
