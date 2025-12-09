// This is free and unencumbered software released into the public domain.

use super::{CameraDevice, CameraOutputTarget, CameraResult};
use core::ptr::null_mut;
use ndk_sys::{
    ACameraDevice_createCaptureRequest, ACameraDevice_request_template, ACaptureRequest,
    ACaptureRequest_addTarget, ACaptureRequest_free, camera_status_t,
};

#[derive(Debug, Default)]
pub struct CaptureRequest {
    pub(crate) handle: *mut ACaptureRequest,
}

impl Drop for CaptureRequest {
    fn drop(&mut self) {
        unsafe { ACaptureRequest_free(self.handle) }
        self.handle = null_mut();
    }
}

impl CaptureRequest {
    pub fn new(device: &CameraDevice) -> CameraResult<Self> {
        let mut result = Self::default();
        result.init(device)?;
        Ok(result)
    }

    fn init(&mut self, device: &CameraDevice) -> CameraResult {
        let status = unsafe {
            ACameraDevice_createCaptureRequest(
                device.handle,
                ACameraDevice_request_template::TEMPLATE_PREVIEW,
                &mut self.handle,
            )
        };
        eprintln!("ACameraDevice_createCaptureRequest={:?}", status); // DEBUG
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }

        Ok(())
    }

    pub fn add_target(&mut self, target: &CameraOutputTarget) -> CameraResult {
        let status = unsafe { ACaptureRequest_addTarget(self.handle, target.handle) };
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(())
    }
}
