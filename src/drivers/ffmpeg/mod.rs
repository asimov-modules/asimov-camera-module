// This is free and unencumbered software released into the public domain.

#[cfg(any(not(target_os = "macos"), not(feature = "avf")))]
pub mod devices;

mod driver;
pub use driver::try_open;
