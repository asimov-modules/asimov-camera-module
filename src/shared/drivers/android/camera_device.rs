// This is free and unencumbered software released into the public domain.

use alloc::ffi::CString;
use core::{mem::zeroed, ptr::null_mut};
use ndk_sys::{ACameraDevice, ACameraDevice_StateCallbacks};

#[derive(Clone, Debug)]
pub struct AndroidCameraDevice {
    pub(crate) device_id: CString,
    pub(crate) device: *mut ACameraDevice,
    pub(crate) device_state_callbacks: ACameraDevice_StateCallbacks,
}

impl Default for AndroidCameraDevice {
    fn default() -> Self {
        Self {
            device_id: CString::new(vec![]).unwrap(),
            device: null_mut(),
            device_state_callbacks: unsafe { zeroed() },
        }
    }
}
