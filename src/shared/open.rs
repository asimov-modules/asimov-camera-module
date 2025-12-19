// This is free and unencumbered software released into the public domain.

use super::{Camera, CameraConfig, CameraError};

pub fn open_camera(
    input_url: impl AsRef<str>,
    config: CameraConfig,
) -> Result<Camera, CameraError> {
    cfg_if::cfg_if! {
        if #[cfg(all(feature = "android", target_os = "android"))] {
            use super::{CameraBackend, CameraEvent, Dispatcher};
            use std::sync::mpsc::sync_channel;

            let input_url = input_url.as_ref().to_string();
            let (events_tx, events_rx) = sync_channel::<CameraEvent>(128);

            let backend = CameraBackend::Android;
            let dispatcher = Dispatcher::new(config.buffer_frames, backend, events_tx.clone());
            let frame_tx = dispatcher.sender();

            let driver = super::drivers::android::AndroidCameraDriver::open(
                input_url,
                config,
                frame_tx,
                events_tx,
            )?;

            Ok(Camera::new(Box::new(driver), dispatcher, events_rx))
        } else if #[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))] {
            use super::{CameraBackend, CameraEvent, Dispatcher};
            use std::sync::mpsc::sync_channel;

            let input_url = input_url.as_ref().to_string();
            let (events_tx, events_rx) = sync_channel::<CameraEvent>(128);

            let backend = CameraBackend::Avf;
            let dispatcher = Dispatcher::new(config.buffer_frames, backend, events_tx.clone());
            let frame_tx = dispatcher.sender();

            let driver = super::drivers::avf::AvfCameraDriver::open(
                input_url,
                config,
                frame_tx,
                events_tx,
            )?;

            Ok(Camera::new(Box::new(driver), dispatcher, events_rx))
        } else if #[cfg(all(feature = "dshow", target_os = "windows"))] {
            use super::{CameraBackend, CameraEvent, Dispatcher};
            use std::sync::mpsc::sync_channel;

            let input_url = input_url.as_ref().to_string();
            let (events_tx, events_rx) = sync_channel::<CameraEvent>(128);

            let backend = CameraBackend::Dshow;
            let dispatcher = Dispatcher::new(config.buffer_frames, backend, events_tx.clone());
            let frame_tx = dispatcher.sender();

            let driver = super::drivers::dshow::DshowCameraDriver::open(
                input_url,
                config,
                frame_tx,
                events_tx,
            )?;

            Ok(Camera::new(Box::new(driver), dispatcher, events_rx))
        } else if #[cfg(all(feature = "v4l2", target_os = "linux"))] {
            use super::{CameraBackend, CameraEvent, Dispatcher};
            use std::sync::mpsc::sync_channel;

            let input_url = input_url.as_ref().to_string();
            let (events_tx, events_rx) = sync_channel::<CameraEvent>(128);

            let backend = CameraBackend::V4l2;
            let dispatcher = Dispatcher::new(config.buffer_frames, backend, events_tx.clone());
            let frame_tx = dispatcher.sender();

            let driver = super::drivers::v4l2::V4l2CameraDriver::open(
                input_url,
                config,
                frame_tx,
                events_tx,
            )?;

            Ok(Camera::new(Box::new(driver), dispatcher, events_rx))
        } else if #[cfg(all(feature = "ffmpeg", any(target_os = "macos", target_os = "linux", target_os = "windows")))] {
            use super::{CameraBackend, CameraEvent, Dispatcher};
            use std::sync::mpsc::sync_channel;

            let input_url = input_url.as_ref().to_string();
            let (events_tx, events_rx) = sync_channel::<CameraEvent>(128);

            let backend = CameraBackend::Ffmpeg;
            let dispatcher = Dispatcher::new(config.buffer_frames, backend, events_tx.clone());
            let frame_tx = dispatcher.sender();

            let driver = super::drivers::ffmpeg::FfmpegCameraDriver::open(
                input_url,
                config,
                frame_tx,
                events_tx,
            )?;

            Ok(Camera::new(Box::new(driver), dispatcher, events_rx))
        } else {
            let _ = input_url;
            let _ = config;
            Err(CameraError::NoDriver)
        }
    }
}
