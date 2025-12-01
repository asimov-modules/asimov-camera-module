// This is free and unencumbered software released into the public domain.

mod camera_device;
pub use camera_device::*;

mod capture_request;
pub use capture_request::*;

mod capture_session;
pub use capture_session::*;

mod image;
pub use image::*;

mod image_reader;
pub use image_reader::*;

mod native_window;
pub use native_window::*;

use crate::shared::{CameraConfig, CameraDriver, CameraError};
use alloc::{borrow::Cow, ffi::CString};
use core::{ffi::CStr, ptr::null_mut};
use ndk_sys::{
    ACameraManager_create, ACameraManager_delete, ACameraManager_deleteCameraIdList,
    ACameraManager_getCameraIdList, ACameraManager_openCamera, android_get_device_api_level,
    camera_status_t,
};
use scopeguard::defer;

#[link(name = "camera2ndk")]
unsafe extern "C" {}

#[derive(Clone, Debug, Default)]
pub struct AndroidCameraDriver {
    pub config: CameraConfig,
    pub api_level: u32,
    #[allow(unused)]
    pub(crate) device: AndroidCameraDevice,
    #[allow(unused)]
    pub(crate) session: Option<AndroidCameraSession>,
}

impl dogma::Named for AndroidCameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "camera2".into()
    }
}

impl Drop for AndroidCameraDriver {
    fn drop(&mut self) {
        self.stop().ok();
    }
}

impl AndroidCameraDriver {
    pub fn open(_input_url: impl AsRef<str>, config: CameraConfig) -> Result<Self, CameraError> {
        unsafe {
            let api_level = android_get_device_api_level() as u32;
            //eprintln!("android_get_device_api_level={}", api_level); // DEBUG

            let camera_manager = ACameraManager_create();
            defer! {
                ACameraManager_delete(camera_manager);
            }

            let mut camera_id_list_ptr = null_mut();
            let status = ACameraManager_getCameraIdList(camera_manager, &mut camera_id_list_ptr);
            if status != camera_status_t::ACAMERA_OK {
                assert!(status != camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER);
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
            eprintln!("ACameraManager_getCameraIdList={:?}", camera_id_strings); // DEBUG

            let mut device = AndroidCameraDevice::default();
            device.device_id = CString::new(camera_id_strings[0].clone()).unwrap();

            let status = ACameraManager_openCamera(
                camera_manager,
                device.device_id.as_ptr(),
                &mut device.device_state_callbacks,
                &mut device.device,
            );
            eprintln!("ACameraManager_openCamera={:?}", status); // DEBUG
            if status != camera_status_t::ACAMERA_OK {
                assert!(status != camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER);
                return Err(CameraError::NoCamera); // TODO
            }

            Ok(AndroidCameraDriver {
                config,
                api_level,
                device,
                session: None,
            })
        }
    }
}

impl CameraDriver for AndroidCameraDriver {
    fn start(&mut self) -> Result<(), CameraError> {
        self.session = Some(AndroidCameraSession::new(&self.device)?);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        self.session = None;
        Ok(())
    }
}
