// This is free and unencumbered software released into the public domain.

use derive_more::Display;
use ndk_sys::camera_status_t;

pub type CameraResult<T = ()> = core::result::Result<T, CameraStatus>;

#[derive(Clone, Copy, Debug, Display)]
#[display("{}", _0.0)]
#[allow(unused)]
pub struct CameraStatus(pub(crate) camera_status_t);

impl core::error::Error for CameraStatus {}

impl Default for CameraStatus {
    fn default() -> Self {
        CameraStatus(camera_status_t::ACAMERA_OK)
    }
}

impl From<camera_status_t> for CameraStatus {
    fn from(input: camera_status_t) -> Self {
        assert!(input != camera_status_t::ACAMERA_ERROR_INVALID_PARAMETER);
        Self(input)
    }
}
