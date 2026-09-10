// This is free and unencumbered software released into the public domain.

use core::fmt;
use ndk_sys::media_status_t;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaStatus(pub media_status_t);

pub type MediaResult<T = ()> = core::result::Result<T, MediaStatus>;

impl MediaStatus {
    #[inline]
    pub const fn ok() -> Self {
        Self(media_status_t::AMEDIA_OK)
    }

    #[inline]
    pub fn is_ok(self) -> bool {
        self.0 == media_status_t::AMEDIA_OK
    }

    #[inline]
    pub fn is_would_block(self) -> bool {
        self.0 == media_status_t::AMEDIA_ERROR_WOULD_BLOCK
    }

    #[inline]
    pub fn is_max_images_acquired(self) -> bool {
        self.0 == media_status_t::AMEDIA_IMGREADER_MAX_IMAGES_ACQUIRED
    }

    #[inline]
    pub fn is_no_buffer_available(self) -> bool {
        self.0 == media_status_t::AMEDIA_IMGREADER_NO_BUFFER_AVAILABLE
    }
}

impl Default for MediaStatus {
    #[inline]
    fn default() -> Self {
        Self::ok()
    }
}

impl From<media_status_t> for MediaStatus {
    #[inline]
    fn from(s: media_status_t) -> Self {
        Self(s)
    }
}

impl fmt::Display for MediaStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "media_status={:?}", self.0)
    }
}

impl core::error::Error for MediaStatus {}
