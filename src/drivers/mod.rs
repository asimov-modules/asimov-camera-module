// This is free and unencumbered software released into the public domain.

use crate::{CameraBackend, CameraError, RawFrameRef};

#[cfg(all(feature = "android", target_os = "android"))]
pub mod android;

#[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))]
pub mod avf;

#[cfg(all(
    feature = "ffmpeg",
    any(target_os = "macos", target_os = "windows", target_os = "linux")
))]
pub mod ffmpeg;

mod config;
pub use config::*;

#[cfg(any(
    all(feature = "android", target_os = "android"),
    all(feature = "avf", any(target_os = "ios", target_os = "macos"))
))]
pub(crate) mod resolution;

pub trait CameraDriver {
    fn backend(&self) -> CameraBackend;

    fn start(&mut self) -> Result<(), CameraError>;
    fn stop(&mut self) -> Result<(), CameraError>;
    fn close(&mut self) -> Result<(), CameraError>;
    fn read_frames(&mut self) -> Result<crossbeam_channel::Receiver<RawFrameRef>, CameraError>;

    #[inline]
    fn preview_info(&self) -> Option<(u32, u32, i32)> {
        None
    }

    #[cfg(all(feature = "mobile-preview", feature = "avf", target_os = "ios"))]
    fn session_handle(&self) -> Result<crate::AvfSessionHandle, CameraError> {
        Err(CameraError::unsupported(
            "session_handle is not supported by this driver",
        ))
    }
}

#[inline]
fn try_driver(
    res: Result<Box<dyn CameraDriver>, CameraError>,
) -> Result<Option<Box<dyn CameraDriver>>, CameraError> {
    match res {
        Ok(d) => Ok(Some(d)),
        Err(e) if e.is_not_applicable() || matches!(e, CameraError::NoDriver) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn open(cfg: &DriverConfig) -> Result<Box<dyn CameraDriver>, CameraError> {
    cfg.validate()?;

    #[cfg(all(feature = "android", target_os = "android"))]
    {
        return match try_driver(android::try_open(cfg))? {
            Some(d) => Ok(d),
            None => Err(CameraError::NoDriver),
        };
    }

    #[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))]
    {
        if let Some(d) = try_driver(avf::try_open(cfg))? {
            return Ok(d);
        }

        #[cfg(target_os = "ios")]
        {
            return Err(CameraError::NoDriver);
        }
    }

    #[cfg(all(
        feature = "ffmpeg",
        any(target_os = "macos", target_os = "windows", target_os = "linux")
    ))]
    {
        return match try_driver(ffmpeg::try_open(cfg))? {
            Some(d) => Ok(d),
            None => Err(CameraError::NoDriver),
        };
    }

    #[allow(unreachable_code)]
    Err(CameraError::NoDriver)
}
