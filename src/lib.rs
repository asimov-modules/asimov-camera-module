// This is free and unencumbered software released into the public domain.

extern crate alloc;

pub mod core;
pub mod shared;

#[cfg(target_os = "ios")]
pub mod ios_test;
