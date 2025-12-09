// This is free and unencumbered software released into the public domain.

use core::{mem::zeroed, ptr::null_mut};
use ndk_sys::{ACameraDevice, ACameraDevice_StateCallbacks, ACameraDevice_close};

#[derive(Clone, Debug)]
pub struct CameraDevice {
    pub(crate) handle: *mut ACameraDevice,
    pub(crate) state_callbacks: ACameraDevice_StateCallbacks,
}

impl Default for CameraDevice {
    fn default() -> Self {
        Self {
            handle: null_mut(),
            state_callbacks: unsafe { zeroed() },
        }
    }
}

impl Drop for CameraDevice {
    fn drop(&mut self) {
        unsafe {
            // See: https://developer.android.com/ndk/reference/group/camera#acameradevice_close
            ACameraDevice_close(self.handle);
            self.handle = null_mut();
        }
    }
}
