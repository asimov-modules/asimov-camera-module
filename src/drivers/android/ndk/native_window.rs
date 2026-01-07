// This is free and unencumbered software released into the public domain.

#![allow(dead_code)]

use core::ptr::null_mut;
use ndk_sys as ndk;

#[derive(Debug)]
pub struct NativeWindow {
    handle: *mut ndk::ANativeWindow,
    owned: bool,
}

impl Default for NativeWindow {
    fn default() -> Self {
        Self {
            handle: null_mut(),
            owned: false,
        }
    }
}

impl NativeWindow {
    #[inline]
    pub fn from_ptr_borrowed(ptr: *mut ndk::ANativeWindow) -> Self {
        Self {
            handle: ptr,
            owned: false,
        }
    }

    #[inline]
    pub fn from_ptr_owned(ptr: *mut ndk::ANativeWindow) -> Self {
        Self {
            handle: ptr,
            owned: true,
        }
    }

    pub fn acquire(&mut self) {
        if self.handle.is_null() || self.owned {
            return;
        }
        unsafe { ndk::ANativeWindow_acquire(self.handle) };
        self.owned = true;
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut ndk::ANativeWindow {
        self.handle
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }

    pub fn width(&self) -> i32 {
        if self.handle.is_null() {
            return 0;
        }
        unsafe { ndk::ANativeWindow_getWidth(self.handle) }
    }

    pub fn height(&self) -> i32 {
        if self.handle.is_null() {
            return 0;
        }
        unsafe { ndk::ANativeWindow_getHeight(self.handle) }
    }

    pub fn format(&self) -> i32 {
        if self.handle.is_null() {
            return 0;
        }
        unsafe { ndk::ANativeWindow_getFormat(self.handle) }
    }

    pub fn set_buffers_geometry(&mut self, width: i32, height: i32, format: i32) -> i32 {
        if self.handle.is_null() {
            return -1;
        }
        unsafe { ndk::ANativeWindow_setBuffersGeometry(self.handle, width, height, format) }
    }

    pub fn into_raw(mut self) -> *mut ndk::ANativeWindow {
        self.owned = false;
        let p = self.handle;
        self.handle = null_mut();
        p
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            if self.owned && !self.handle.is_null() {
                ndk::ANativeWindow_release(self.handle);
                self.handle = null_mut();
                self.owned = false;
            }
        }
    }
}
