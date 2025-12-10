// This is free and unencumbered software released into the public domain.

mod config;
pub use config::*;

mod driver;
pub use driver::*;

pub mod drivers {
    /// Camera driver using FFmpeg.
    #[cfg(all(
        feature = "ffmpeg",
        any(target_os = "macos", target_os = "linux", target_os = "windows")
    ))]
    pub mod ffmpeg;

    /// Camera driver using the NDK on Android.
    #[cfg(target_os = "android")]
    pub mod android;

    /// Camera driver using AVFoundation on iOS and macOS.
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub mod avf;

    /// Camera driver using Dshow on Windows.
    #[cfg(target_os = "windows")]
    pub mod dshow;

    /// Camera driver using V4L2 on Linux.
    #[cfg(target_os = "linux")]
    pub mod v4l2;
}

mod error;
pub use error::*;

mod frame;
pub use frame::*;

mod open;
pub use open::*;
