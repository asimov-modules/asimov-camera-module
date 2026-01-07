// This is free and unencumbered software released into the public domain.

pub mod devices;

mod driver;
pub use driver::try_open;

mod ndk;
