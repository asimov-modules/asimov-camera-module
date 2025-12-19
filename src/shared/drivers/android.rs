// This is free and unencumbered software released into the public domain.

mod camera_capture_session;
pub use camera_capture_session::*;

mod camera_device;
pub use camera_device::*;

mod camera_manager;
pub use camera_manager::*;

mod camera_output_target;
pub use camera_output_target::*;

mod camera_status;
pub use camera_status::*;

mod capture_request;
pub use capture_request::*;

mod capture_session_output;
pub use capture_session_output::*;

mod capture_session_output_container;
pub use capture_session_output_container::*;

mod image;
pub use image::*;

mod image_reader;
pub use image_reader::*;

mod media_status;
pub use media_status::*;

mod native_window;
pub use native_window::*;

use crate::shared::{
    CameraBackend, CameraConfig, CameraDriver, CameraError, CameraEvent, Frame, FrameMsg,
    try_send_frame,
};
use alloc::{borrow::Cow, ffi::CString};
use bytes::Bytes;
use core::{ffi::CStr, ptr::null_mut};
use ndk_sys::{
    ACameraManager_create, ACameraManager_delete, ACameraManager_deleteCameraIdList,
    ACameraManager_getCameraIdList, ACameraManager_openCamera, android_get_device_api_level,
    camera_status_t,
};
use scopeguard::defer;
use std::any::Any;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::SyncSender,
};

#[link(name = "camera2ndk")]
unsafe extern "C" {}

#[link(name = "mediandk")]
unsafe extern "C" {}

#[link(name = "binder_ndk")]
unsafe extern "C" {}

#[derive(Debug)]
pub struct AndroidCameraDriver {
    pub config: CameraConfig,
    pub api_level: u32,
    #[allow(unused)]
    pub(crate) device: CameraDevice,
    #[allow(unused)]
    pub(crate) session: Option<CameraCaptureSession>,

    frame_tx: SyncSender<FrameMsg>,
    events_tx: SyncSender<CameraEvent>,
    running: Arc<AtomicBool>,
}

impl dogma::Named for AndroidCameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "camera2".into()
    }
}

impl Drop for AndroidCameraDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl AndroidCameraDriver {
    pub fn open(
        _input_url: impl AsRef<str>,
        config: CameraConfig,
        frame_tx: SyncSender<FrameMsg>,
        events_tx: SyncSender<CameraEvent>,
    ) -> Result<Self, CameraError> {
        unsafe {
            let api_level = android_get_device_api_level() as u32;

            let camera_manager = ACameraManager_create();
            defer! {
                ACameraManager_delete(camera_manager);
            }

            let mut camera_id_list_ptr = null_mut();
            let status = ACameraManager_getCameraIdList(camera_manager, &mut camera_id_list_ptr);
            if status != camera_status_t::ACAMERA_OK {
                return Err(CameraError::NoCamera);
            }
            defer! {
                ACameraManager_deleteCameraIdList(camera_id_list_ptr);
            }

            let camera_id_list = &*camera_id_list_ptr;
            if camera_id_list.numCameras < 1 {
                return Err(CameraError::NoCamera);
            }

            let camera_ids = core::slice::from_raw_parts(
                camera_id_list.cameraIds,
                camera_id_list.numCameras as usize,
            );
            let camera_id_strings: Vec<String> = camera_ids
                .iter()
                .map(|p| CStr::from_ptr(*p).to_str().unwrap_or("").to_string())
                .collect();

            if config.diagnostics {
                let _ = events_tx.try_send(CameraEvent::Warning {
                    backend: CameraBackend::Android,
                    message: format!("ACameraManager_getCameraIdList={camera_id_strings:?}"),
                });
            }

            let mut device = CameraDevice::default();
            let device_id = CString::new(camera_id_strings[0].clone()).unwrap();

            let status = ACameraManager_openCamera(
                camera_manager,
                device_id.as_ptr(),
                &mut device.state_callbacks,
                &mut device.handle,
            );

            if config.diagnostics {
                let _ = events_tx.try_send(CameraEvent::Warning {
                    backend: CameraBackend::Android,
                    message: format!("ACameraManager_openCamera status={status:?}"),
                });
            }

            if status != camera_status_t::ACAMERA_OK {
                return Err(CameraError::NoCamera);
            }

            Ok(AndroidCameraDriver {
                config,
                api_level,
                device,
                session: None,
                frame_tx,
                events_tx,
                running: Arc::new(AtomicBool::new(false)),
            })
        }
    }

    fn emit_frame(&self, frame: Frame) {
        try_send_frame(
            &self.frame_tx,
            &self.events_tx,
            CameraBackend::Android,
            frame,
        );
    }
}

impl CameraDriver for AndroidCameraDriver {
    fn backend(&self) -> CameraBackend {
        CameraBackend::Android
    }

    fn start(&mut self) -> Result<(), CameraError> {
        let session_output_container = CaptureSessionOutputContainer::new().unwrap();
        self.session =
            Some(CameraCaptureSession::open(&self.device, &session_output_container).unwrap()); // FIXME

        Err(CameraError::unsupported(
            "android camera backend not implemented",
        ))
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        self.session = None;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
