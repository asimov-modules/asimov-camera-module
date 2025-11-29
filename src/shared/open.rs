// This is free and unencumbered software released into the public domain.

use super::{CameraDriver, CameraError};

pub fn open(_input_url: impl AsRef<str>) -> Result<Box<dyn CameraDriver>, CameraError> {
    if cfg!(feature = "ffmpeg") {
        #[cfg(feature = "ffmpeg")]
        return Ok(Box::new(super::drivers::ffmpeg::FfmpegCameraDriver {}));
    }
    if cfg!(any(target_os = "ios", target_os = "macos")) {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        return Ok(Box::new(
            super::drivers::avfoundation::AvFoundationCameraDriver {},
        ));
    }
    if cfg!(target_os = "android") {
        #[cfg(target_os = "android")]
        return Ok(Box::new(super::drivers::camera2::Camera2CameraDriver {}));
    }
    if cfg!(target_os = "windows") {
        #[cfg(target_os = "windows")]
        return Ok(Box::new(super::drivers::dshow::DshowCameraDriver {}));
    }
    if cfg!(target_os = "linux") {
        #[cfg(target_os = "linux")]
        return Ok(Box::new(super::drivers::v4l2::V4l2CameraDriver {}));
    }
    Err(CameraError::NoDriver)
}
