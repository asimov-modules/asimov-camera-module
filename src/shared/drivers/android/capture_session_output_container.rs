// This is free and unencumbered software released into the public domain.

use super::{CameraResult, CaptureSessionOutput};
use core::ptr::null_mut;
use ndk_sys::{
    ACaptureSessionOutputContainer, ACaptureSessionOutputContainer_add,
    ACaptureSessionOutputContainer_create, ACaptureSessionOutputContainer_free, camera_status_t,
};

#[derive(Debug, Default)]
pub struct CaptureSessionOutputContainer {
    pub(crate) handle: *mut ACaptureSessionOutputContainer,
}

impl Drop for CaptureSessionOutputContainer {
    fn drop(&mut self) {
        // See: https://developer.android.com/ndk/reference/group/camera#acapturesessionoutputcontainer_free
        unsafe { ACaptureSessionOutputContainer_free(self.handle) };
        self.handle = null_mut();
    }
}

impl CaptureSessionOutputContainer {
    pub fn new() -> CameraResult<Self> {
        // See: https://developer.android.com/ndk/reference/group/camera#acapturesessionoutputcontainer_create
        let mut result = Self::default();
        let status = unsafe { ACaptureSessionOutputContainer_create(&mut result.handle) };
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(result)
    }

    pub fn add(&mut self, target: &CaptureSessionOutput) -> CameraResult {
        // See: https://developer.android.com/ndk/reference/group/camera#acapturesessionoutputcontainer_add
        let status = unsafe { ACaptureSessionOutputContainer_add(self.handle, target.handle) };
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(())
    }
}
