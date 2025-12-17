// This is free and unencumbered software released into the public domain.

use super::{CameraConfig, CameraDriver, CameraError};

pub fn open_camera(
    input_url: impl AsRef<str>,
    config: CameraConfig,
) -> Result<Box<dyn CameraDriver>, CameraError> {
    // Use underscore bindings so builds that compile no backend branches
    // do not warn about unused locals.
    let _input_url = input_url.as_ref();
    let _config = config;

    // Android native
    #[cfg(all(feature = "android", target_os = "android"))]
    {
        return Ok(Box::new(
            super::drivers::android::AndroidCameraDriver::open(_input_url, _config)?,
        ));
    }

    // Apple native
    #[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))]
    {
        return Ok(Box::new(
            super::drivers::avf::AvfCameraDriver::open(_input_url, _config)?,
        ));
    }

    // Windows native
    #[cfg(all(feature = "dshow", target_os = "windows"))]
    {
        return Ok(Box::new(super::drivers::dshow::DshowCameraDriver { config: _config }));
    }

    // Linux native
    #[cfg(all(feature = "v4l2", target_os = "linux"))]
    {
        return Ok(Box::new(super::drivers::v4l2::V4l2CameraDriver { config: _config }));
    }

    // FFmpeg fallback (only when no native backend is compiled for this target)
    #[cfg(all(
        feature = "ffmpeg",
        not(all(feature = "android", target_os = "android")),
        not(all(feature = "avf", any(target_os = "ios", target_os = "macos"))),
        not(all(feature = "dshow", target_os = "windows")),
        not(all(feature = "v4l2", target_os = "linux")),
    ))]
    {
        return Ok(Box::new(super::drivers::ffmpeg::FfmpegCameraDriver {
            config: _config,
            process: None,
        }));
    }

    // No backend available at all (this function only compiles in that case)
    #[cfg(all(
        not(all(feature = "android", target_os = "android")),
        not(all(feature = "avf", any(target_os = "ios", target_os = "macos"))),
        not(all(feature = "dshow", target_os = "windows")),
        not(all(feature = "v4l2", target_os = "linux")),
        not(feature = "ffmpeg"),
    ))]
    {
        Err(CameraError::NoDriver)
    }
}
