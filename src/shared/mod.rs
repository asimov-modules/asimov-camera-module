// This is free and unencumbered software released into the public domain.

mod config;
pub use config::*;

mod driver;
pub use driver::*;

pub mod drivers {
    #[cfg(feature = "ffmpeg")]
    pub mod ffmpeg;

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub mod avfoundation;

    #[cfg(target_os = "android")]
    pub mod camera2;

    #[cfg(target_os = "windows")]
    pub mod dshow;

    #[cfg(target_os = "linux")]
    pub mod v4l2;
}

mod error;
pub use error::*;

mod open;
pub use open::*;
