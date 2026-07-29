// This is free and unencumbered software released into the public domain.

use core::ptr::null_mut;
use ndk_sys as ndk;

use super::CameraResult;
use super::camera_device::CameraDevice;
use super::camera_status::CameraStatus;

#[derive(Debug)]
pub struct CameraCaptureSession {
    handle: *mut ndk::ACameraCaptureSession,
    _state_callbacks: Box<ndk::ACameraCaptureSession_stateCallbacks>,
}

impl CameraCaptureSession {
    pub fn open(
        device: &CameraDevice,
        outputs: *mut ndk::ACaptureSessionOutputContainer,
        mut state_callbacks: Box<ndk::ACameraCaptureSession_stateCallbacks>,
    ) -> CameraResult<Self> {
        if device.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }
        if outputs.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }

        let mut handle: *mut ndk::ACameraCaptureSession = null_mut();

        let st = unsafe {
            ndk::ACameraDevice_createCaptureSession(
                device.as_ptr(),
                outputs,
                state_callbacks.as_mut(),
                &mut handle,
            )
        };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() || handle.is_null() {
            return Err(stw);
        }

        Ok(Self {
            handle,
            _state_callbacks: state_callbacks,
        })
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut ndk::ACameraCaptureSession {
        self.handle
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }

    pub fn set_repeating_request(
        &self,
        request: *mut ndk::ACaptureRequest,
        out_sequence_id: Option<&mut i32>,
    ) -> CameraResult {
        if self.handle.is_null() || request.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }

        let mut seq_id_tmp: i32 = 0;
        let seq_ptr: *mut i32 = match out_sequence_id {
            Some(r) => r as *mut i32,
            None => &mut seq_id_tmp as *mut i32,
        };

        let mut reqs: [*mut ndk::ACaptureRequest; 1] = [request];

        let st = unsafe {
            ndk::ACameraCaptureSession_setRepeatingRequest(
                self.handle,
                null_mut(),
                reqs.len() as i32,
                reqs.as_mut_ptr(),
                seq_ptr,
            )
        };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() {
            return Err(stw);
        }
        Ok(())
    }

    pub fn stop_repeating(&self) -> CameraResult {
        if self.handle.is_null() {
            return Ok(());
        }
        let st = unsafe { ndk::ACameraCaptureSession_stopRepeating(self.handle) };
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
        unsafe {
            ndk::ACameraCaptureSession_close(self.handle);
        }
        self.handle = null_mut();
    }
}

impl Drop for CameraCaptureSession {
    fn drop(&mut self) {
        self.close();
    }
}
