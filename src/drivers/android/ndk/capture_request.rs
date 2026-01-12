// This is free and unencumbered software released into the public domain.

use core::ptr::null_mut;
use ndk_sys as ndk;

use super::camera_device::CameraDevice;
use super::camera_output_target::CameraOutputTarget;
use super::camera_status::{CameraResult, CameraStatus};

#[derive(Debug, Default)]
pub struct CaptureRequest {
    handle: *mut ndk::ACaptureRequest,
}

impl CaptureRequest {
    pub fn new(
        device: &CameraDevice,
        template: ndk::ACameraDevice_request_template,
    ) -> CameraResult<Self> {
        if device.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }

        let mut out = Self::default();

        let st = unsafe {
            ndk::ACameraDevice_createCaptureRequest(device.as_ptr(), template, &mut out.handle)
        };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() || out.handle.is_null() {
            return Err(stw);
        }

        Ok(out)
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut ndk::ACaptureRequest {
        self.handle
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }

    pub fn add_target(&self, target: &CameraOutputTarget) -> CameraResult {
        if self.handle.is_null() || target.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }

        let st = unsafe { ndk::ACaptureRequest_addTarget(self.handle, target.as_ptr()) };
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
        unsafe { ndk::ACaptureRequest_free(self.handle) };
        self.handle = null_mut();
    }
}

impl Drop for CaptureRequest {
    fn drop(&mut self) {
        self.close();
    }
}
