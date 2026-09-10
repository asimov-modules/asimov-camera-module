// This is free and unencumbered software released into the public domain.

#![cfg(all(
    feature = "mobile-preview",
    any(target_os = "ios", target_os = "android")
))]

use core::ffi::c_void;
use core::ptr::NonNull;

pub mod handles {
    use super::*;

    #[cfg(all(feature = "android", target_os = "android"))]
    #[derive(Clone, Copy, Debug)]
    pub struct AndroidPreviewTarget(pub NonNull<c_void>);

    #[cfg(all(feature = "android", target_os = "android"))]
    impl AndroidPreviewTarget {
        pub unsafe fn from_nonnull_ptr(ptr: NonNull<c_void>) -> Self {
            Self(ptr)
        }

        #[inline]
        pub fn as_ptr(self) -> *mut c_void {
            self.0.as_ptr()
        }
    }

    #[cfg(all(feature = "avf", target_os = "ios"))]
    #[derive(Clone, Copy, Debug)]
    pub struct AvfSessionHandle(pub NonNull<c_void>);

    #[cfg(all(feature = "avf", target_os = "ios"))]
    impl AvfSessionHandle {
        pub unsafe fn from_nonnull_ptr(ptr: NonNull<c_void>) -> Self {
            Self(ptr)
        }

        #[inline]
        pub fn as_ptr(self) -> *mut c_void {
            self.0.as_ptr()
        }
    }
}

#[cfg(all(feature = "android", target_os = "android"))]
pub use handles::AndroidPreviewTarget;

#[cfg(all(feature = "avf", target_os = "ios"))]
pub use handles::AvfSessionHandle;
