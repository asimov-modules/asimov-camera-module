// This is free and unencumbered software released into the public domain.

mod config;
pub use config::*;

mod driver;
pub use driver::*;

pub mod drivers {
    /// Camera driver for FFmpeg.
    #[cfg(feature = "ffmpeg")]
    pub mod ffmpeg;

    /// Camera driver for AVFoundation on iOS and macOS.
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub mod avf;

    /// Camera driver for Camera2 on Android.
    #[cfg(target_os = "android")]
    pub mod camera2;

    /// Camera driver for Dshow on Windows.
    #[cfg(target_os = "windows")]
    pub mod dshow;

    /// Camera driver for V4L2 on Linux.
    #[cfg(target_os = "linux")]
    pub mod v4l2;
}

mod error;
pub use error::*;

mod open;
pub use open::*;
