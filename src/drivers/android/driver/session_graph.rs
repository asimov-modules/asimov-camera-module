// This is free and unencumbered software released into the public domain.

use ndk_sys as ndk;

use crate::CameraError;

use crate::drivers::android::ndk::{
    CameraCaptureSession, CameraDevice, CameraOutputTarget, CaptureRequest, CaptureSessionOutput,
    CaptureSessionOutputContainer, NativeWindow,
};

pub struct SessionGraph {
    session: CameraCaptureSession,
    request: CaptureRequest,
    output_container: CaptureSessionOutputContainer,
    preview_output: CaptureSessionOutput,
    capture_output: CaptureSessionOutput,
    preview_target: CameraOutputTarget,
    capture_target: CameraOutputTarget,
}

impl SessionGraph {
    pub fn new(
        dev: &CameraDevice,
        preview_window: &mut NativeWindow,
        capture_window_ptr: *mut ndk::ANativeWindow,
        session_callbacks: Box<ndk::ACameraCaptureSession_stateCallbacks>,
    ) -> Result<Self, CameraError> {
        if preview_window.as_ptr().is_null() {
            return Err(CameraError::invalid_config("preview window is null"));
        }
        if capture_window_ptr.is_null() {
            return Err(CameraError::invalid_config(
                "capture (ImageReader) window is null",
            ));
        }

        let request =
            CaptureRequest::new(dev, ndk::ACameraDevice_request_template::TEMPLATE_RECORD)
                .map_err(|st| {
                    CameraError::driver("android: ACameraDevice_createCaptureRequest", st)
                })?;

        let output_container = CaptureSessionOutputContainer::new().map_err(|st| {
            CameraError::driver("android: ACaptureSessionOutputContainer_create", st)
        })?;

        let preview_output = CaptureSessionOutput::new(preview_window.as_ptr()).map_err(|st| {
            CameraError::driver("android: ACaptureSessionOutput_create(preview)", st)
        })?;

        let capture_output = CaptureSessionOutput::new(capture_window_ptr).map_err(|st| {
            CameraError::driver("android: ACaptureSessionOutput_create(capture)", st)
        })?;

        output_container.add(&preview_output).map_err(|st| {
            CameraError::driver("android: ACaptureSessionOutputContainer_add(preview)", st)
        })?;

        output_container.add(&capture_output).map_err(|st| {
            CameraError::driver("android: ACaptureSessionOutputContainer_add(capture)", st)
        })?;

        let preview_target = CameraOutputTarget::new(preview_window.as_ptr()).map_err(|st| {
            CameraError::driver("android: ACameraOutputTarget_create(preview)", st)
        })?;

        let capture_target = CameraOutputTarget::new(capture_window_ptr).map_err(|st| {
            CameraError::driver("android: ACameraOutputTarget_create(capture)", st)
        })?;

        request
            .add_target(&preview_target)
            .map_err(|st| CameraError::driver("android: ACaptureRequest_addTarget(preview)", st))?;

        request
            .add_target(&capture_target)
            .map_err(|st| CameraError::driver("android: ACaptureRequest_addTarget(capture)", st))?;

        let session = CameraCaptureSession::open(dev, output_container.as_ptr(), session_callbacks)
            .map_err(|st| CameraError::driver("android: ACameraDevice_createCaptureSession", st))?;

        Ok(Self {
            session,
            request,
            output_container,
            preview_output,
            capture_output,
            preview_target,
            capture_target,
        })
    }

    pub fn start_repeating(&self) -> Result<(), CameraError> {
        let mut seq_id: i32 = 0;

        self.session
            .set_repeating_request(self.request.as_ptr(), Some(&mut seq_id))
            .map_err(|st| {
                CameraError::driver("android: ACameraCaptureSession_setRepeatingRequest", st)
            })?;

        Ok(())
    }

    pub fn stop_repeating(&self) -> Result<(), CameraError> {
        self.session.stop_repeating().map_err(|st| {
            CameraError::driver("android: ACameraCaptureSession_stopRepeating", st)
        })?;

        Ok(())
    }

    pub fn close(&mut self) {
        self.session.close();
        self.request.close();
        self.capture_target.close();
        self.preview_target.close();
        self.capture_output.close();
        self.preview_output.close();
        self.output_container.close();
    }
}

impl Drop for SessionGraph {
    fn drop(&mut self) {
        self.close();
    }
}
