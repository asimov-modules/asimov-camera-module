// This is free and unencumbered software released into the public domain.

use core::fmt;
use ndk_sys::camera_status_t;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CameraStatus(pub camera_status_t);

pub type CameraResult<T = ()> = core::result::Result<T, CameraStatus>;

impl CameraStatus {
    #[inline]
    pub const fn ok() -> Self {
        Self(camera_status_t::ACAMERA_OK)
    }

    #[inline]
    pub const fn code(self) -> i32 {
        (self.0).0
    }

    #[inline]
    pub fn is_ok(self) -> bool {
        self.0 == camera_status_t::ACAMERA_OK
    }

    #[inline]
    pub fn is_disconnected(self) -> bool {
        self.0 == camera_status_t::ACAMERA_ERROR_CAMERA_DISCONNECTED
    }

    #[inline]
    pub fn is_not_enough_memory(self) -> bool {
        self.0 == camera_status_t::ACAMERA_ERROR_NOT_ENOUGH_MEMORY
    }

    #[inline]
    pub fn is_permission_denied(self) -> bool {
        self.0 == camera_status_t::ACAMERA_ERROR_PERMISSION_DENIED
    }

    #[inline]
    pub fn is_camera_in_use(self) -> bool {
        self.0 == camera_status_t::ACAMERA_ERROR_CAMERA_IN_USE
    }

    #[inline]
    pub fn is_max_cameras_in_use(self) -> bool {
        self.0 == camera_status_t::ACAMERA_ERROR_MAX_CAMERA_IN_USE
    }
}

impl Default for CameraStatus {
    #[inline]
    fn default() -> Self {
        Self::ok()
    }
}

impl From<camera_status_t> for CameraStatus {
    #[inline]
    fn from(s: camera_status_t) -> Self {
        Self(s)
    }
}

impl fmt::Display for CameraStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "camera_status={}", self.code())
    }
}

impl core::error::Error for CameraStatus {}
