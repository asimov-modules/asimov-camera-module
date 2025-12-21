// This is free and unencumbered software released into the public domain.

use crate::shared::{open_camera, Camera, CameraConfig, CameraError, Frame, FrameSink, PixelFormat};
use core::ffi::{c_char, c_double, c_void};
use core::ptr;
use objc2::rc::Retained;
use objc2_av_foundation::AVCaptureSession;
use std::ffi::CStr;

pub type AsimovCameraFrameCallback = extern "C" fn(
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    stride: u32,
    user_data: *mut c_void,
);

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AsimovCameraErrorCode {
    Ok = 0,
    NoDriver = 1,
    NoCamera = 2,
    DriverError = 3,
    InvalidArgument = 4,
}

impl From<CameraError> for AsimovCameraErrorCode {
    fn from(err: CameraError) -> Self {
        match err {
            CameraError::NoDriver => AsimovCameraErrorCode::NoDriver,
            CameraError::NoCamera => AsimovCameraErrorCode::NoCamera,
            _ => AsimovCameraErrorCode::DriverError,
        }
    }
}

pub struct AsimovCameraHandle {
    pub camera: Camera,
}

#[derive(Copy, Clone)]
struct SessionPtr(*mut c_void);

unsafe impl Send for SessionPtr {}

fn device_c_str_to_string(device: *const c_char) -> Result<String, AsimovCameraErrorCode> {
    if device.is_null() {
        return Ok(String::new());
    }

    unsafe {
        CStr::from_ptr(device)
            .to_str()
            .map(|s| s.to_owned())
            .map_err(|_| AsimovCameraErrorCode::InvalidArgument)
    }
}

fn make_sink(frame_cb: AsimovCameraFrameCallback, user_data: *mut c_void) -> FrameSink {
    let user_bits = user_data as usize;

    std::sync::Arc::new(move |frame: Frame| {
        let user_ptr = user_bits as *mut c_void;

        frame_cb(
            frame.data.as_ptr(),
            frame.data.len(),
            frame.width,
            frame.height,
            frame.stride,
            user_ptr,
        );
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_open(
    device: *const c_char,
    width: u32,
    height: u32,
    fps: c_double,
    frame_callback: AsimovCameraFrameCallback,
    user_data: *mut c_void,
    out_handle: *mut *mut c_void,
) -> AsimovCameraErrorCode {
    if out_handle.is_null() {
        return AsimovCameraErrorCode::InvalidArgument;
    }

    let device_str = match device_c_str_to_string(device) {
        Ok(s) => s,
        Err(e) => return e,
    };

    let mut config = CameraConfig::new(width, height, fps);
    config.pixel_format = Some(PixelFormat::Bgra8);
    config.device = if device_str.is_empty() { None } else { Some(device_str) };

    let mut camera = match open_camera("", config) {
        Ok(c) => c,
        Err(e) => return e.into(),
    };

    camera.add_sink(make_sink(frame_callback, user_data));

    let handle = Box::new(AsimovCameraHandle { camera });

    unsafe {
        *out_handle = Box::into_raw(handle) as *mut c_void;
    }

    AsimovCameraErrorCode::Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_start(handle: *mut c_void) -> AsimovCameraErrorCode {
    if handle.is_null() {
        return AsimovCameraErrorCode::InvalidArgument;
    }

    let handle = unsafe { &mut *(handle as *mut AsimovCameraHandle) };
    handle
        .camera
        .start()
        .map(|_| AsimovCameraErrorCode::Ok)
        .unwrap_or_else(Into::into)
}

#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_stop(handle: *mut c_void) -> AsimovCameraErrorCode {
    if handle.is_null() {
        return AsimovCameraErrorCode::InvalidArgument;
    }

    let handle = unsafe { &mut *(handle as *mut AsimovCameraHandle) };
    handle
        .camera
        .stop()
        .map(|_| AsimovCameraErrorCode::Ok)
        .unwrap_or_else(Into::into)
}

#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(handle as *mut AsimovCameraHandle));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_get_session(handle: *mut c_void) -> *mut c_void {
    if handle.is_null() {
        return ptr::null_mut();
    }

    let handle = unsafe { &mut *(handle as *mut AsimovCameraHandle) };

    let Some(driver) = handle
        .camera
        .driver_as::<crate::shared::drivers::avf::AvfCameraDriver>()
    else {
        return ptr::null_mut();
    };

    let Some(bound) = driver.session_bound() else {
        return ptr::null_mut();
    };

    let result: SessionPtr = bound.get_on_main(|s: &Retained<AVCaptureSession>| {
        let ptr: *const AVCaptureSession = &**s;
        SessionPtr(ptr as *mut c_void)
    });

    result.0
}
