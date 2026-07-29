// This is free and unencumbered software released into the public domain.

//! FFmpeg-subprocess capture for Linux/Windows — a temporary stand-in until
//! native v4l2/Media Foundation drivers exist. Deliberately standalone: this
//! module mirrors the shape of `nativecam`'s public API (same type/method
//! names) so `src/bin/*.rs` can share one code path, but shares no code with
//! it — deleting this whole module, plus the two `#[cfg(...)]` branches that
//! reference it in the binaries, is meant to be enough to remove FFmpeg
//! entirely once native Linux/Windows drivers land in `nativecam`.

mod config;
pub use config::*;

mod devices;
pub use devices::*;

mod driver;
pub use driver::*;

mod error;
pub use error::*;

mod frame;
pub use frame::*;

mod subscribe;
pub use subscribe::*;
