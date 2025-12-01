// This is free and unencumbered software released into the public domain.

use super::{AndroidCameraDevice, AndroidCaptureRequest};
use crate::shared::CameraError;
use core::ptr::null_mut;
use ndk_sys::{
    ACameraCaptureSession, ACameraCaptureSession_capture, ACameraCaptureSession_close,
    ACameraCaptureSession_stateCallbacks, ACameraDevice_createCaptureSession,
    ACaptureSessionOutputContainer, camera_status_t,
};
//use scopeguard::defer;

#[derive(Clone, Debug)]
pub struct AndroidCameraSession {
    handle: *mut ACameraCaptureSession,
    //output: *mut ACaptureSessionOutput,
    output_container: *mut ACaptureSessionOutputContainer,
    state_callbacks: ACameraCaptureSession_stateCallbacks,
}

impl Default for AndroidCameraSession {
    fn default() -> Self {
        Self {
            handle: null_mut(),
            //output: null_mut(),
            output_container: null_mut(),
            state_callbacks: ACameraCaptureSession_stateCallbacks {
                context: null_mut(),
                onClosed: None,
                onReady: None,
                onActive: None,
            },
        }
    }
}

impl Drop for AndroidCameraSession {
    fn drop(&mut self) {
        self.close()
    }
}

impl AndroidCameraSession {
    pub fn new(device: &AndroidCameraDevice) -> Result<Self, CameraError> {
        let mut result = Self::default();
        let status = unsafe {
            ACameraDevice_createCaptureSession(
                device.device,
                result.output_container,
                &result.state_callbacks,
                &mut result.handle,
            )
        };
        eprintln!("ACameraDevice_createCaptureSession={:?}", status); // DEBUG
        if status != camera_status_t::ACAMERA_OK {
            assert!(status != camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::NoCamera); // TODO
        }
        Ok(result)
    }

    pub fn capture(&mut self, request: &AndroidCaptureRequest) -> Result<(), CameraError> {
        let mut requests = request.handle;
        let status = unsafe {
            ACameraCaptureSession_capture(self.handle, null_mut(), 1, &mut requests, null_mut())
        };
        eprintln!("ACameraCaptureSession_capture={:?}", status); // DEBUG
        if status != camera_status_t::ACAMERA_OK {
            assert!(status != camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::NoCamera); // TODO
        }
        Ok(())
    }

    pub fn close(&mut self) {
        unsafe { ACameraCaptureSession_close(self.handle) }
    }
}
