// This is free and unencumbered software released into the public domain.

use core::ffi::c_void;
use core::ptr::null_mut;

use ndk_sys::{
    AImageReader, AImageReader_ImageListener, AImageReader_acquireLatestImage, AImageReader_delete,
    AImageReader_getFormat, AImageReader_getHeight, AImageReader_getWidth, AImageReader_getWindow,
    AImageReader_new, AImageReader_setImageListener, ANativeWindow, media_status_t,
};

use super::{Image, MediaResult, MediaStatus, NativeWindow};

#[derive(Debug)]
pub struct ImageReader {
    handle: *mut AImageReader,
    listener: Option<Box<AImageReader_ImageListener>>,
}

impl Default for ImageReader {
    fn default() -> Self {
        Self {
            handle: null_mut(),
            listener: None,
        }
    }
}

impl ImageReader {
    pub fn new(dimensions: (u32, u32), format: i32, max_images: i32) -> MediaResult<Self> {
        let (width, height) = dimensions;

        let mut this = Self::default();
        let status = unsafe {
            AImageReader_new(
                width as _,
                height as _,
                format,
                max_images,
                &mut this.handle,
            )
        };

        if status != media_status_t::AMEDIA_OK || this.handle.is_null() {
            return Err(MediaStatus::from(status));
        }

        Ok(this)
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut AImageReader {
        self.handle
    }

    pub fn get_window(&self) -> MediaResult<NativeWindow> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }

        let mut win: *mut ANativeWindow = null_mut();
        let status = unsafe { AImageReader_getWindow(self.handle, &mut win) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        if win.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_UNKNOWN.into());
        }

        Ok(NativeWindow::from_ptr_borrowed(win))
    }

    pub fn set_image_listener(
        &mut self,
        context: *mut c_void,
        cb: extern "C" fn(*mut c_void, *mut AImageReader),
    ) -> MediaResult {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }

        let mut listener = Box::new(AImageReader_ImageListener {
            context,
            onImageAvailable: Some(cb),
        });

        let status = unsafe { AImageReader_setImageListener(self.handle, listener.as_mut()) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }

        self.listener = Some(listener);
        Ok(())
    }

    pub fn acquire_latest_image(&self) -> MediaResult<Image> {
        unsafe { acquire_latest_image_from_raw(self.handle) }
    }

    pub fn get_width(&self) -> MediaResult<u32> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }

        let mut result = 0;
        let status = unsafe { AImageReader_getWidth(self.handle, &mut result) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result as _)
    }

    pub fn get_height(&self) -> MediaResult<u32> {
        if self.handle.is_null() {
            return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
        }

        let mut result = 0;
        let status = unsafe { AImageReader_getHeight(self.handle, &mut result) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result as _)
    }

    pub fn close(&mut self) {
        unsafe {
            if !self.handle.is_null() {
                AImageReader_delete(self.handle);
                self.handle = null_mut();
            }
        }
        self.listener = None;
    }
}

impl Drop for ImageReader {
    fn drop(&mut self) {
        self.close();
    }
}

pub unsafe fn acquire_latest_image_from_raw(reader: *mut AImageReader) -> MediaResult<Image> {
    if reader.is_null() {
        return Err(media_status_t::AMEDIA_ERROR_INVALID_PARAMETER.into());
    }

    let mut result = Image::default();

    let status = unsafe { AImageReader_acquireLatestImage(reader, &mut result.handle) };

    if status != media_status_t::AMEDIA_OK {
        return Err(MediaStatus::from(status));
    }
    if result.handle.is_null() {
        return Err(media_status_t::AMEDIA_ERROR_UNKNOWN.into());
    }

    Ok(result)
}
