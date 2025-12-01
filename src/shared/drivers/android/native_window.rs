// This is free and unencumbered software released into the public domain.

use core::ptr::null_mut;
use ndk_sys::{ANativeWindow, ANativeWindow_release};

#[derive(Debug, Default)]
pub struct AndroidNativeWindow {
    pub(crate) handle: *mut ANativeWindow,
}

impl Drop for AndroidNativeWindow {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.handle);
            self.handle = null_mut();
        }
    }
}

impl AndroidNativeWindow {}
