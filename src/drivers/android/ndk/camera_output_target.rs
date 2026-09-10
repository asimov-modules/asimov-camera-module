// This is free and unencumbered software released into the public domain.

use core::ptr::null_mut;
use ndk_sys as ndk;

use super::camera_status::{CameraResult, CameraStatus};

#[derive(Debug, Default)]
pub struct CameraOutputTarget {
    handle: *mut ndk::ACameraOutputTarget,
}

impl CameraOutputTarget {
    #[inline]
    pub fn as_ptr(&self) -> *mut ndk::ACameraOutputTarget {
        self.handle
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }

    pub fn new(window: *mut ndk::ANativeWindow) -> CameraResult<Self> {
        if window.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }

        let mut out = Self::default();
        let st = unsafe { ndk::ACameraOutputTarget_create(window, &mut out.handle) };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() || out.handle.is_null() {
            return Err(stw);
        }

        Ok(out)
    }

    pub fn close(&mut self) {
        if self.handle.is_null() {
            return;
        }
        unsafe {
            ndk::ACameraOutputTarget_free(self.handle);
        }
        self.handle = null_mut();
    }
}

impl Drop for CameraOutputTarget {
    fn drop(&mut self) {
        self.close();
    }
}
