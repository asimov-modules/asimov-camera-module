// This is free and unencumbered software released into the public domain.

use super::{CameraConfig, CameraDriver, CameraError, FrameCallback};

pub fn open_camera(
    input_url: impl AsRef<str>,
    config: CameraConfig,
    callback: FrameCallback,
) -> Result<Box<dyn CameraDriver>, CameraError> {
    #[cfg(all(
        feature = "ffmpeg",
        any(target_os = "macos", target_os = "linux", target_os = "windows")
    ))]
    {
        return Ok(Box::new(super::drivers::ffmpeg::FfmpegCameraDriver {
            config,
            process: None,
        }));
    }

    #[cfg(target_os = "android")]
    {
        return Ok(Box::new(
            super::drivers::android::AndroidCameraDriver::open(input_url, config)?,
        ));
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    {
        return Ok(Box::new(super::drivers::avf::AvfCameraDriver::open(
            input_url, config, callback,
        )?));
    }

    #[cfg(target_os = "windows")]
    {
        return Ok(Box::new(super::drivers::dshow::DshowCameraDriver {
            config,
        }));
    }

    #[cfg(target_os = "linux")]
    {
        return Ok(Box::new(super::drivers::v4l2::V4l2CameraDriver { config }));
    }

    #[allow(unreachable_code)]
    Err(CameraError::NoDriver)
}
