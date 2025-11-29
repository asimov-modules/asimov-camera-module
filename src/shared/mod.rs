// This is free and unencumbered software released into the public domain.

mod driver;
pub use driver::*;

pub mod drivers {
    #[cfg(feature = "ffmpeg")]
    pub mod ffmpeg;

    #[cfg(all(not(feature = "ffmpeg"), any(target_os = "macos", target_os = "ios")))]
    pub mod avfoundation;

    #[cfg(all(not(feature = "ffmpeg"), target_os = "android"))]
    pub mod camera2;

    #[cfg(all(not(feature = "ffmpeg"), target_os = "windows"))]
    pub mod dshow;

    #[cfg(all(not(feature = "ffmpeg"), target_os = "linux"))]
    pub mod v4l2;
}

mod error;
pub use error::*;
