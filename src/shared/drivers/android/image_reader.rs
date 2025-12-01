// This is free and unencumbered software released into the public domain.

use super::AndroidImage;
use crate::shared::CameraError;
use core::ptr::null_mut;
use ndk_sys::{
    AImageReader, AImageReader_acquireLatestImage, AImageReader_delete, AImageReader_getFormat,
    AImageReader_getHeight, AImageReader_getWidth, AImageReader_new, media_status_t,
};

#[derive(Debug, Default)]
pub struct AndroidImageReader {
    pub(crate) handle: *mut AImageReader,
}

impl Drop for AndroidImageReader {
    fn drop(&mut self) {
        unsafe {
            AImageReader_delete(self.handle);
            self.handle = null_mut();
        }
    }
}

impl AndroidImageReader {
    pub fn new(dimensions: (u32, u32), format: i32) -> Result<Self, CameraError> {
        let (width, height) = dimensions;
        let mut result = Self::default();
        let status;
        unsafe {
            status = AImageReader_new(width as _, height as _, format, 2, &mut result.handle);
        }
        if status != media_status_t::AMEDIA_OK {
            assert!(status != media_status_t::AMEDIA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::Other); // TODO
        }
        Ok(result)
    }

    pub fn get_format(&self) -> Result<i32, CameraError> {
        let mut result = 0;
        let status;
        unsafe {
            status = AImageReader_getFormat(self.handle, &mut result);
        }
        if status != media_status_t::AMEDIA_OK {
            assert!(status != media_status_t::AMEDIA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::Other); // TODO
        }
        Ok(result)
    }

    pub fn get_dimensions(&self) -> Result<(u32, u32), CameraError> {
        Ok((self.get_width()?, self.get_height()?))
    }

    pub fn get_width(&self) -> Result<u32, CameraError> {
        let mut result = 0;
        let status;
        unsafe {
            status = AImageReader_getWidth(self.handle, &mut result);
        }
        if status != media_status_t::AMEDIA_OK {
            assert!(status != media_status_t::AMEDIA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::Other); // TODO
        }
        Ok(result as _)
    }

    pub fn get_height(&self) -> Result<u32, CameraError> {
        let mut result = 0;
        let status;
        unsafe {
            status = AImageReader_getHeight(self.handle, &mut result);
        }
        if status != media_status_t::AMEDIA_OK {
            assert!(status != media_status_t::AMEDIA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::Other); // TODO
        }
        Ok(result as _)
    }

    pub fn acquire_latest_image(&self) -> Result<AndroidImage, CameraError> {
        let mut result = AndroidImage::default();
        let status;
        unsafe {
            status = AImageReader_acquireLatestImage(self.handle, &mut result.handle);
        }
        if status != media_status_t::AMEDIA_OK {
            assert!(status != media_status_t::AMEDIA_ERROR_INVALID_PARAMETER);
            return Err(CameraError::Other); // TODO
        }
        Ok(result)
    }
}
