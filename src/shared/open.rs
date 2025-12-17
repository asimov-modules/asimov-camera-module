// This is free and unencumbered software released into the public domain.

use super::{CameraConfig, CameraDriver, CameraError, FrameCallback};

pub fn open_camera(
    input_url: impl AsRef<str>,
    config: CameraConfig,
    callback: FrameCallback,
) -> Result<Box<dyn CameraDriver>, CameraError> {
    let input_url = input_url.as_ref();

    #[cfg(all(feature = "android", target_os = "android"))]
    {
        let driver =
            super::drivers::android::AndroidCameraDriver::open(input_url, config, callback)?;
        return Ok(Box::new(driver));
    }

    #[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))]
    {
        let driver = super::drivers::avf::AvfCameraDriver::open(input_url, config, callback)?;
        return Ok(Box::new(driver));
    }

    #[cfg(all(feature = "dshow", target_os = "windows"))]
    {
        let driver = super::drivers::dshow::DshowCameraDriver::open(input_url, config, callback)?;
        return Ok(Box::new(driver));
    }

    #[cfg(all(feature = "v4l2", target_os = "linux"))]
    {
        let driver = super::drivers::v4l2::V4l2CameraDriver::open(input_url, config, callback)?;
        return Ok(Box::new(driver));
    }

    #[cfg(all(
        feature = "ffmpeg",
        any(target_os = "macos", target_os = "linux", target_os = "windows"),
        not(all(feature = "android", target_os = "android")),
        not(all(feature = "avf", any(target_os = "ios", target_os = "macos"))),
        not(all(feature = "dshow", target_os = "windows")),
        not(all(feature = "v4l2", target_os = "linux")),
    ))]
    {
        let driver = super::drivers::ffmpeg::FfmpegCameraDriver::open(input_url, config, callback)?;
        return Ok(Box::new(driver));
    }

    #[cfg(all(
        not(all(feature = "android", target_os = "android")),
        not(all(feature = "avf", any(target_os = "ios", target_os = "macos"))),
        not(all(feature = "dshow", target_os = "windows")),
        not(all(feature = "v4l2", target_os = "linux")),
        not(all(
            feature = "ffmpeg",
            any(target_os = "macos", target_os = "linux", target_os = "windows")
        )),
    ))]
    {
        let _ = callback;
        let _ = input_url;
        let _ = config;
        Err(CameraError::NoDriver)
    }
}
