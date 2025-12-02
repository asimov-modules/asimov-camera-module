// This is free and unencumbered software released into the public domain.

use super::{Image, MediaResult, NativeWindow};
use core::ffi::c_void;
use core::ptr::null_mut;
use ndk_sys::{
    AImageReader, AImageReader_ImageListener, AImageReader_acquireLatestImage, AImageReader_delete,
    AImageReader_getFormat, AImageReader_getHeight, AImageReader_getWidth, AImageReader_getWindow,
    AImageReader_new, AImageReader_setImageListener, media_status_t,
};

#[derive(Debug)]
pub struct ImageReader {
    pub(crate) handle: *mut AImageReader,
    pub(crate) image_listener: AImageReader_ImageListener,
}

impl Default for ImageReader {
    fn default() -> Self {
        Self {
            handle: null_mut(),
            image_listener: AImageReader_ImageListener {
                context: null_mut(),
                onImageAvailable: None,
            },
        }
    }
}

impl Drop for ImageReader {
    fn drop(&mut self) {
        unsafe { AImageReader_delete(self.handle) }
        self.handle = null_mut();
    }
}

impl ImageReader {
    /// See: https://developer.android.com/ndk/reference/group/media#aimagereader_new
    pub fn new(dimensions: (u32, u32), format: i32) -> MediaResult<Self> {
        let (width, height) = dimensions;
        let mut this = Self::default();

        let status =
            unsafe { AImageReader_new(width as _, height as _, format, 2, &mut this.handle) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }

        unsafe extern "C" fn on_image_available(
            _context: *mut c_void,
            image_reader: *mut AImageReader,
        ) {
            eprintln!("ImageReader#on_image_available"); // TODO
            let mut result = Image::default();
            let _status =
                unsafe { AImageReader_acquireLatestImage(image_reader, &mut result.handle) };
        }

        let this_ptr: *mut ImageReader = &mut this as *mut _;
        this.image_listener.context = this_ptr as *mut c_void;
        this.image_listener.onImageAvailable = Some(on_image_available);

        let status =
            unsafe { AImageReader_setImageListener(this.handle, &mut this.image_listener) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }

        Ok(this)
    }

    /// See: https://developer.android.com/ndk/reference/group/media#aimagereader_getformat
    pub fn get_format(&self) -> MediaResult<i32> {
        let mut result = 0;
        let status = unsafe { AImageReader_getFormat(self.handle, &mut result) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result)
    }

    pub fn get_dimensions(&self) -> MediaResult<(u32, u32)> {
        Ok((self.get_width()?, self.get_height()?))
    }

    /// See: https://developer.android.com/ndk/reference/group/media#aimagereader_getwidth
    pub fn get_width(&self) -> MediaResult<u32> {
        let mut result = 0;
        let status = unsafe { AImageReader_getWidth(self.handle, &mut result) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result as _)
    }

    /// See: https://developer.android.com/ndk/reference/group/media#aimagereader_getheight
    pub fn get_height(&self) -> MediaResult<u32> {
        let mut result = 0;
        let status = unsafe { AImageReader_getHeight(self.handle, &mut result) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result as _)
    }

    /// See: https://developer.android.com/ndk/reference/group/media#aimagereader_getwindow
    pub fn get_window(&self) -> MediaResult<NativeWindow> {
        let mut result = NativeWindow {
            owned: false, // the ANativeWindow is managed by this image reader
            ..Default::default()
        };
        let status = unsafe { AImageReader_getWindow(self.handle, &mut result.handle) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result)
    }

    /// See: https://developer.android.com/ndk/reference/group/media#aimagereader_setimagelistener
    pub(crate) fn _set_image_listener(&mut self) -> MediaResult {
        Ok(()) // TODO
    }

    /// See: https://developer.android.com/ndk/reference/group/media#aimagereader_acquirelatestimage
    pub fn acquire_latest_image(&self) -> MediaResult<Image> {
        let mut result = Image::default();
        let status = unsafe { AImageReader_acquireLatestImage(self.handle, &mut result.handle) };
        if status != media_status_t::AMEDIA_OK {
            return Err(status.into());
        }
        Ok(result)
    }
}
