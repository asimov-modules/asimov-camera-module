// This is free and unencumbered software released into the public domain.

use super::{CameraDevice, CameraResult, CaptureRequest, CaptureSessionOutputContainer};
use core::ffi::c_void;
use core::ptr::null_mut;
use ndk_sys::{
    ACameraCaptureSession, ACameraCaptureSession_capture, ACameraCaptureSession_close,
    ACameraCaptureSession_setRepeatingRequest, ACameraCaptureSession_stateCallbacks,
    ACameraCaptureSession_stopRepeating, ACameraDevice_createCaptureSession, camera_status_t,
};

#[derive(Clone, Debug)]
pub struct CameraCaptureSession {
    handle: *mut ACameraCaptureSession,
    state_callbacks: ACameraCaptureSession_stateCallbacks,
}

impl Default for CameraCaptureSession {
    fn default() -> Self {
        Self {
            handle: null_mut(),
            state_callbacks: ACameraCaptureSession_stateCallbacks {
                context: null_mut(),
                onClosed: None,
                onReady: None,
                onActive: None,
            },
        }
    }
}

impl Drop for CameraCaptureSession {
    fn drop(&mut self) {
        self.close()
    }
}

impl CameraCaptureSession {
    pub fn open(
        device: &CameraDevice,
        outputs: &CaptureSessionOutputContainer,
    ) -> CameraResult<Self> {
        let mut result = Self::default();
        result.init(device, outputs)?;
        Ok(result)
    }

    fn init(
        &mut self,
        device: &CameraDevice,
        outputs: &CaptureSessionOutputContainer,
    ) -> CameraResult {
        self.state_callbacks.context = (self as *mut _) as *mut c_void;

        unsafe extern "C" fn on_ready(_context: *mut c_void, session: *mut ACameraCaptureSession) {
            eprintln!("CameraCaptureSession#on_ready: session={:?}", session);
        }
        self.state_callbacks.onReady = Some(on_ready);

        unsafe extern "C" fn on_active(_context: *mut c_void, session: *mut ACameraCaptureSession) {
            eprintln!("CameraCaptureSession#on_active: session={:?}", session);
        }
        self.state_callbacks.onActive = Some(on_active);

        unsafe extern "C" fn on_closed(_context: *mut c_void, session: *mut ACameraCaptureSession) {
            eprintln!("CameraCaptureSession#on_closed: session={:?}", session);
        }
        self.state_callbacks.onClosed = Some(on_closed);

        let status = unsafe {
            ACameraDevice_createCaptureSession(
                device.handle,
                //self.output_container,
                outputs.handle,
                &self.state_callbacks,
                &mut self.handle,
            )
        };
        eprintln!("ACameraDevice_createCaptureSession={:?}", status); // DEBUG
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }

        Ok(())
    }

    pub fn capture(&mut self, request: &CaptureRequest) -> CameraResult {
        let mut requests = request.handle;
        let status = unsafe {
            ACameraCaptureSession_capture(self.handle, null_mut(), 1, &mut requests, null_mut())
        };
        eprintln!("ACameraCaptureSession_capture={:?}", status); // DEBUG
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(())
    }

    /// See: https://developer.android.com/ndk/reference/group/camera#acameracapturesession_setrepeatingrequest
    pub fn set_repeating_request(&mut self, request: &CaptureRequest) -> CameraResult {
        let mut requests = request.handle;
        let status = unsafe {
            ACameraCaptureSession_setRepeatingRequest(
                self.handle,
                null_mut(),
                1,
                &mut requests,
                null_mut(),
            )
        };
        eprintln!("ACameraCaptureSession_setRepeatingRequest={:?}", status); // DEBUG
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(())
    }

    /// See: https://developer.android.com/ndk/reference/group/camera#acameracapturesession_stoprepeating
    pub fn stop_repeating(&mut self) -> CameraResult {
        let status = unsafe { ACameraCaptureSession_stopRepeating(self.handle) };
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }
        Ok(())
    }

    pub fn close(&mut self) {
        unsafe { ACameraCaptureSession_close(self.handle) }
    }
}
