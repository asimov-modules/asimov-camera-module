// This is free and unencumbered software released into the public domain.

use super::{CameraResult, NativeWindow};
use core::ptr::null_mut;
use ndk_sys::{
    ACameraOutputTarget, ACameraOutputTarget_create, ACameraOutputTarget_free, camera_status_t,
};

#[derive(Debug, Default)]
pub struct CameraOutputTarget {
    pub(crate) handle: *mut ACameraOutputTarget,
}

impl Drop for CameraOutputTarget {
    fn drop(&mut self) {
        unsafe {
            ACameraOutputTarget_free(self.handle);
            self.handle = null_mut();
        }
    }
}

impl CameraOutputTarget {
    pub fn new(window: &NativeWindow) -> CameraResult<Self> {
        // See: https://developer.android.com/ndk/reference/group/camera#acameraoutputtarget_create
        let mut result = Self::default();
        let status = unsafe { ACameraOutputTarget_create(window.handle, &mut result.handle) };
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(result)
    }
}
