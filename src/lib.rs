// This is free and unencumbered software released into the public domain.

//! ASIMOV camera module. Capture comes from two independent sources,
//! selected per-platform at compile time — see `src/bin/*.rs`:
//! - Android/iOS/macOS: the standalone [`nativecam`] crate.
//! - Linux/Windows: [`ffmpeg`], a temporary subprocess-based stand-in until
//!   native drivers exist for those two platforms.

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub mod ffmpeg;
