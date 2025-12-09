// This is free and unencumbered software released into the public domain.

use core::ptr::null_mut;
use ndk_sys::{ANativeWindow, ANativeWindow_acquire, ANativeWindow_release};

#[derive(Debug, Default)]
pub struct NativeWindow {
    pub(crate) handle: *mut ANativeWindow,
    pub(crate) owned: bool,
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                ANativeWindow_release(self.handle);
                self.handle = null_mut();
            }
        }
    }
}

impl NativeWindow {
    pub fn acquire(&self) {
        unsafe { ANativeWindow_acquire(self.handle) }
    }
}
