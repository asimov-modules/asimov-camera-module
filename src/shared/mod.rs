// This is free and unencumbered software released into the public domain.

mod config;
pub use config::*;

mod driver;
pub use driver::*;

pub mod drivers {
    /// Camera driver using FFmpeg.
    #[cfg(feature = "ffmpeg")]
    pub mod ffmpeg;

    /// Camera driver using the NDK on Android.
    #[cfg(all(feature = "android", target_os = "android"))]
    pub mod android;

    /// Camera driver using AVFoundation on iOS and macOS.
    #[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))]
    pub mod avf;

    /// Camera driver using DShow on Windows.
    #[cfg(all(feature = "dshow", target_os = "windows"))]
    pub mod dshow;

    /// Camera driver using V4L2 on Linux.
    #[cfg(all(feature = "v4l2", target_os = "linux"))]
    pub mod v4l2;
}

mod error;
pub use error::*;

mod open;
pub use open::*;
