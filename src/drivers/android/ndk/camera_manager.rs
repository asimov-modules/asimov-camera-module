// This is free and unencumbered software released into the public domain.

use core::ffi::c_char;
use core::ptr::null_mut;
use ndk_sys as ndk;

use super::camera_device::CameraDevice;
use super::camera_status::{CameraResult, CameraStatus};

#[derive(Debug)]
pub struct CameraManager {
    handle: *mut ndk::ACameraManager,
}

#[derive(Debug)]
pub struct CameraIdList {
    handle: *mut ndk::ACameraIdList,
}

impl CameraIdList {
    #[inline]
    pub fn len(&self) -> usize {
        if self.handle.is_null() {
            return 0;
        }
        unsafe { (*self.handle).numCameras as usize }
    }

    #[inline]
    pub fn id_ptr(&self, idx: usize) -> *const c_char {
        if self.handle.is_null() {
            return core::ptr::null();
        }
        unsafe {
            let list_ref = &*self.handle;
            if idx >= (list_ref.numCameras as usize) {
                return core::ptr::null();
            }
            *list_ref.cameraIds.add(idx) as *const c_char
        }
    }
}

impl Drop for CameraIdList {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                ndk::ACameraManager_deleteCameraIdList(self.handle);
                self.handle = null_mut();
            }
        }
    }
}

impl CameraManager {
    pub fn new() -> CameraResult<Self> {
        let handle = unsafe { ndk::ACameraManager_create() };
        if handle.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_UNKNOWN,
            ));
        }
        Ok(Self { handle })
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut ndk::ACameraManager {
        self.handle
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.handle.is_null()
    }

    pub fn open_camera(
        &self,
        camera_id: *const c_char,
        mut callbacks: Box<ndk::ACameraDevice_StateCallbacks>,
    ) -> CameraResult<CameraDevice> {
        if self.handle.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_CAMERA_DISCONNECTED,
            ));
        }
        if camera_id.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER,
            ));
        }

        let mut dev_ptr: *mut ndk::ACameraDevice = null_mut();

        let st = unsafe {
            ndk::ACameraManager_openCamera(self.handle, camera_id, callbacks.as_mut(), &mut dev_ptr)
        };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() || dev_ptr.is_null() {
            return Err(stw);
        }

        Ok(CameraDevice::new(dev_ptr, callbacks))
    }

    pub fn get_characteristics(
        &self,
        camera_id: *const c_char,
        out_metadata: *mut *mut ndk::ACameraMetadata,
    ) -> ndk::camera_status_t {
        unsafe {
            ndk::ACameraManager_getCameraCharacteristics(self.handle, camera_id, out_metadata)
        }
    }

    pub fn list_camera_ids(&self) -> CameraResult<CameraIdList> {
        if self.handle.is_null() {
            return Err(CameraStatus::from(
                ndk::camera_status_t::ACAMERA_ERROR_CAMERA_DISCONNECTED,
            ));
        }

        let mut id_list: *mut ndk::ACameraIdList = null_mut();
        let st = unsafe { ndk::ACameraManager_getCameraIdList(self.handle, &mut id_list) };

        let stw = CameraStatus::from(st);
        if !stw.is_ok() || id_list.is_null() {
            return Err(stw);
        }

        Ok(CameraIdList { handle: id_list })
    }
}

impl Drop for CameraManager {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                ndk::ACameraManager_delete(self.handle);
                self.handle = null_mut();
            }
        }
    }
}
