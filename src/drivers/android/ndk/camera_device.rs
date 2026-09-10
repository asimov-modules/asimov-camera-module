// This is free and unencumbered software released into the public domain.

use core::ptr::null_mut;
use ndk_sys as ndk;

#[derive(Debug)]
pub struct CameraDevice {
    handle: *mut ndk::ACameraDevice,
    _state_callbacks: Box<ndk::ACameraDevice_StateCallbacks>,
}

impl CameraDevice {
    #[inline]
    pub fn new(
        handle: *mut ndk::ACameraDevice,
        state_callbacks: Box<ndk::ACameraDevice_StateCallbacks>,
    ) -> Self {
        Self {
            handle,
            _state_callbacks: state_callbacks,
        }
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut ndk::ACameraDevice {
        self.handle
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }
}

impl Drop for CameraDevice {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                ndk::ACameraDevice_close(self.handle);
                self.handle = null_mut();
            }
        }
    }
}
