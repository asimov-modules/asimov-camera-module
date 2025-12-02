// This is free and unencumbered software released into the public domain.

use super::{CameraDevice, CameraResult};
use alloc::ffi::CString;
use core::{ffi::CStr, ptr::null_mut};
//use core::{mem::zeroed, ptr::null_mut};
use ndk_sys::{
    ACameraManager, ACameraManager_create, ACameraManager_delete,
    ACameraManager_deleteCameraIdList, ACameraManager_getCameraIdList, ACameraManager_openCamera,
    camera_status_t,
};
use scopeguard::defer;

#[derive(Debug, Default)]
pub struct CameraManager {
    pub(crate) handle: *mut ACameraManager,
}

impl Drop for CameraManager {
    fn drop(&mut self) {
        unsafe {
            ACameraManager_delete(self.handle);
            self.handle = null_mut();
        }
    }
}

impl CameraManager {
    pub fn new() -> Self {
        Self {
            handle: unsafe { ACameraManager_create() },
        }
    }

    pub fn get_camera_ids(&self) -> CameraResult<Vec<String>> {
        let mut list_ptr = null_mut();
        let status = unsafe { ACameraManager_getCameraIdList(self.handle, &mut list_ptr) };
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }

        defer! {
            unsafe { ACameraManager_deleteCameraIdList(list_ptr); }
        }

        let list = unsafe { &*list_ptr };
        if list.numCameras < 1 {
            return Ok(Vec::new()); // no devices
        }

        let ids = unsafe { core::slice::from_raw_parts(list.cameraIds, list.numCameras as usize) };

        let result: Vec<String> = ids
            .iter()
            .map(|p| unsafe { CStr::from_ptr(*p).to_str().unwrap_or("").to_string() })
            .collect();

        Ok(result)
    }

    pub fn open_camera(&self, id: impl AsRef<str>) -> CameraResult<CameraDevice> {
        let id = CString::new(String::from(id.as_ref())).unwrap();

        let mut device = CameraDevice::default();
        let status = unsafe {
            ACameraManager_openCamera(
                self.handle,
                id.as_ptr(),
                &mut device.state_callbacks,
                &mut device.handle,
            )
        };
        eprintln!("ACameraManager_openCamera={:?}", status); // DEBUG
        if status != camera_status_t::ACAMERA_OK {
            return Err(status.into());
        }

        Ok(device)
    }
}
