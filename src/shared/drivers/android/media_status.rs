// This is free and unencumbered software released into the public domain.

use derive_more::Display;
use ndk_sys::media_status_t;

pub type MediaResult<T = ()> = core::result::Result<T, MediaStatus>;

#[derive(Clone, Copy, Debug, Display)]
#[display("{}", _0.0)]
#[allow(unused)]
pub struct MediaStatus(pub(crate) media_status_t);

impl core::error::Error for MediaStatus {}

impl Default for MediaStatus {
    fn default() -> Self {
        MediaStatus(media_status_t::AMEDIA_OK)
    }
}

impl From<media_status_t> for MediaStatus {
    fn from(input: media_status_t) -> Self {
        assert!(input != media_status_t::AMEDIA_ERROR_INVALID_PARAMETER);
        Self(input)
    }
}
