// This is free and unencumbered software released into the public domain.

use super::{Camera, CameraConfig, CameraError};

#[allow(unused_imports)]
use super::{CameraBackend, CameraEvent, Dispatcher};
#[allow(unused_imports)]
use std::sync::mpsc::sync_channel;

pub fn open_camera(
    input_url: impl AsRef<str>,
    config: CameraConfig,
) -> Result<Camera, CameraError> {
    // Defining the macro inside the function limits its scope
    // and helps suppress "unused" warnings when no features are enabled.
    #[allow(unused_macros)]
    macro_rules! init_camera {
        ($driver_type:ty, $backend:expr, $url:expr, $config:expr) => {{
            let (events_tx, events_rx) = sync_channel::<CameraEvent>(128);
            let dispatcher = Dispatcher::new($config.buffer_frames, $backend, events_tx.clone());
            let frame_tx = dispatcher.sender();

            let driver =
                <$driver_type>::open($url.as_ref().to_string(), $config, frame_tx, events_tx)?;

            Ok(Camera::new(Box::new(driver), dispatcher, events_rx))
        }};
    }

    cfg_if::cfg_if! {
        if #[cfg(all(feature = "android", target_os = "android"))] {
            init_camera!(super::drivers::android::AndroidCameraDriver, CameraBackend::Android, input_url, config)
        } else if #[cfg(all(feature = "ffmpeg", any(target_os = "macos", target_os = "linux", target_os = "windows")))] {
            init_camera!(super::drivers::ffmpeg::FfmpegCameraDriver, CameraBackend::Ffmpeg, input_url, config)
        } else if #[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))] {
            init_camera!(super::drivers::avf::AvfCameraDriver, CameraBackend::Avf, input_url, config)
        } else if #[cfg(all(feature = "dshow", target_os = "windows"))] {
            init_camera!(super::drivers::dshow::DshowCameraDriver, CameraBackend::Dshow, input_url, config)
        } else if #[cfg(all(feature = "v4l2", target_os = "linux"))] {
            init_camera!(super::drivers::v4l2::V4l2CameraDriver, CameraBackend::V4l2, input_url, config)
        } else {
            let _ = (input_url, config);
            Err(CameraError::NoDriver)
        }
    }
}
