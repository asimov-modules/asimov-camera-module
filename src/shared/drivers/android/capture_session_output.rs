// This is free and unencumbered software released into the public domain.

use super::{CameraResult, NativeWindow};
use core::ptr::null_mut;
use ndk_sys::{
    ACaptureSessionOutput, ACaptureSessionOutput_create, ACaptureSessionOutput_free,
    camera_status_t,
};

#[derive(Debug, Default)]
pub struct CaptureSessionOutput {
    pub(crate) handle: *mut ACaptureSessionOutput,
}

impl Drop for CaptureSessionOutput {
    fn drop(&mut self) {
        unsafe {
            ACaptureSessionOutput_free(self.handle);
            self.handle = null_mut();
        }
    }
}

impl CaptureSessionOutput {
    pub fn new(window: &NativeWindow) -> CameraResult<Self> {
        // See: https://developer.android.com/ndk/reference/group/camera#acapturesessionoutput_create
        let mut result = Self::default();
        let status = unsafe { ACaptureSessionOutput_create(window.handle, &mut result.handle) };
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(result)
    }
}
