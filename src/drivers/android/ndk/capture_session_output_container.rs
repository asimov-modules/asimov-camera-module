// This is free and unencumbered software released into the public domain.

#![allow(dead_code)]

use core::ptr::null_mut;
use ndk_sys as ndk;

use super::camera_status::{CameraResult, CameraStatus};
use super::capture_session_output::CaptureSessionOutput;

#[derive(Debug, Default)]
pub struct CaptureSessionOutputContainer {
    handle: *mut ndk::ACaptureSessionOutputContainer,
}

impl CaptureSessionOutputContainer {
    #[inline]
    pub fn as_ptr(&self) -> *mut ndk::ACaptureSessionOutputContainer {
        self.handle
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }

    pub fn new() -> CameraResult<Self> {
        let mut out = Self::default();
        let st = unsafe { ndk::ACaptureSessionOutputContainer_create(&mut out.handle) };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() || out.handle.is_null() {
            return Err(stw);
        }

        Ok(out)
    }

    pub fn add(&self, output: &CaptureSessionOutput) -> CameraResult {
        if self.handle.is_null() || output.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }

        let st = unsafe { ndk::ACaptureSessionOutputContainer_add(self.handle, output.as_ptr()) };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() {
            return Err(stw);
        }

        Ok(())
    }

    pub fn close(&mut self) {
        if self.handle.is_null() {
            return;
        }
        unsafe { ndk::ACaptureSessionOutputContainer_free(self.handle) };
        self.handle = null_mut();
    }
}

impl Drop for CaptureSessionOutputContainer {
    fn drop(&mut self) {
        self.close();
    }
}
