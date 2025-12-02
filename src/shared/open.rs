// This is free and unencumbered software released into the public domain.

use super::{CameraConfig, CameraDriver, CameraError, FrameCallback};

pub fn open_camera(
    input_url: impl AsRef<str>,
    config: CameraConfig,
    callback: FrameCallback,
) -> Result<Box<dyn CameraDriver>, CameraError> {
    if cfg!(feature = "ffmpeg") {
        #[cfg(feature = "ffmpeg")]
        return Ok(Box::new(super::drivers::ffmpeg::FfmpegCameraDriver {
            config,
            process: None,
        }));
    }

    if cfg!(target_os = "android") {
        #[cfg(target_os = "android")]
        return Ok(Box::new(
            super::drivers::android::AndroidCameraDriver::open(input_url, config)?,
        ));
    }

    if cfg!(any(target_os = "ios", target_os = "macos")) {
        #[cfg(any(target_os = "ios", target_os = "macos"))]
        return Ok(Box::new(super::drivers::avf::AvfCameraDriver::open(
            input_url, config, callback,
        )?));
    }

    if cfg!(target_os = "windows") {
        #[cfg(target_os = "windows")]
        return Ok(Box::new(super::drivers::dshow::DshowCameraDriver {
            config,
        }));
    }

    if cfg!(target_os = "linux") {
        #[cfg(target_os = "linux")]
        return Ok(Box::new(super::drivers::v4l2::V4l2CameraDriver { config }));
    }

    #[allow(unreachable_code)]
    Err(CameraError::NoDriver)
}
