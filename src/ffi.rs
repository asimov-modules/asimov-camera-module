// This is free and unencumbered software released into the public domain.

// iOS-only FFI for the AVFoundation-backed camera driver.
// Exposed as a C ABI for use from Swift/Objective-C.

use crate::shared::{
    CameraConfig, CameraDriver, CameraError, Frame, drivers::avf::AvfCameraDriver,
};
use core::ffi::{c_char, c_double, c_void};
use core::ptr;
use objc2::rc::Retained;
use objc2_av_foundation::AVCaptureSession;
use std::ffi::CStr;

/// C-style frame callback type that Swift will implement.
///
/// IMPORTANT:
/// - `data` points to BGRA pixels.
/// - The pointer is only valid for the duration of the callback.
/// - If Swift needs to keep it, it MUST copy the buffer.
pub type AsimovCameraFrameCallback = extern "C" fn(
    data: *const u8,
    len: usize,
    width: u32,
    height: u32,
    stride: u32,
    user_data: *mut c_void,
);

/// Error codes returned by the FFI API.
///
/// These map directly from [`CameraError`] plus a few FFI-specific cases.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AsimovCameraErrorCode {
    /// No error.
    Ok = 0,
    /// No suitable driver available for the current platform.
    NoDriver = 1,
    /// No camera device was found or could be opened.
    NoCamera = 2,
    /// Underlying driver/AVFoundation error.
    DriverError = 3,
    /// Invalid arguments passed via the FFI boundary.
    InvalidArgument = 4,
}

/// Map internal `CameraError` to FFI error codes.
impl From<CameraError> for AsimovCameraErrorCode {
    fn from(err: CameraError) -> Self {
        match err {
            CameraError::NoDriver => AsimovCameraErrorCode::NoDriver,
            CameraError::NoCamera => AsimovCameraErrorCode::NoCamera,
            CameraError::DriverError => AsimovCameraErrorCode::DriverError,
        }
    }
}

/// Internal handle we keep on the Rust side.
///
/// Swift only sees this as an opaque `void*`.
/// Here we store the concrete `AvfCameraDriver` so we can reach its session.
pub struct AsimovCameraHandle {
    pub driver: AvfCameraDriver,
}

/// Wrapper for a raw session pointer so it can be returned from `get_on_main`.
#[derive(Copy, Clone)]
struct SessionPtr(*mut c_void);

unsafe impl Send for SessionPtr {}

/// Convert a C string pointer into a Rust `String`.
///
/// - `NULL` is treated as an empty string.
/// - Returns `InvalidArgument` if the C string is not valid UTF-8.
fn device_c_str_to_string(device: *const c_char) -> Result<String, AsimovCameraErrorCode> {
    // NULL = "use default device"
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

/// Wrap the C frame callback in a Rust [`FrameCallback`].
///
/// The Rust side deals in [`Frame`], and forwards the underlying BGRA buffer
/// and metadata into the FFI callback.
fn make_rust_frame_callback(
    frame_cb: AsimovCameraFrameCallback,
    user_data: *mut c_void,
) -> crate::shared::FrameCallback {
    // Store the pointer as plain bits so the closure only captures a `usize`.
    // `usize` is `Send + Sync`, so this satisfies the `FrameCallback` alias.
    let user_bits = user_data as usize;

    Box::new(move |frame: Frame| {
        // Reconstruct the pointer inside the closure. This local does NOT
        // participate in the Send/Sync analysis of the closure captures.
        let user_ptr = user_bits as *mut c_void;

        // IMPORTANT:
        // - `frame.data.as_ptr()` is only valid during this call.
        // - If Swift wants to keep it, it must copy.
        frame_cb(
            frame.data.as_ptr(),
            frame.data.len(),
            frame.width as u32,
            frame.height as u32,
            frame.stride as u32,
            user_ptr,
        );
    })
}

/// Open a camera and create a handle.
///
/// Parameters:
/// - `device`: C string for device ID or name; NULL or "" = default device.
/// - `width`, `height`, `fps`: desired configuration.
/// - `frame_callback`: called from Rust whenever a video frame is available.
/// - `user_data`: opaque pointer passed back into `frame_callback`.
/// - `out_handle`: [out] pointer to receive an opaque camera handle.
///
/// Threading:
/// - Must be called on the **main thread**. If not, returns `DriverError`.
///
/// Return:
/// - `AsimovCameraErrorCode::Ok` on success, error code otherwise.
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

    let config = CameraConfig::new(device_str, width, height, fps);
    let cb = make_rust_frame_callback(frame_callback, user_data);

    // iOS-specific: directly create the AVFoundation driver.
    match AvfCameraDriver::open("", config, cb) {
        Ok(driver) => {
            let handle = Box::new(AsimovCameraHandle { driver });

            unsafe {
                *out_handle = Box::into_raw(handle) as *mut c_void;
            }

            AsimovCameraErrorCode::Ok
        },
        Err(e) => e.into(),
    }
}

/// Start capturing frames.
///
/// Threading:
/// - May be called from any thread. Internally hops to main thread for
///   AVFoundation work.
///
/// Returns:
/// - `Ok` on success, or an appropriate error code.
#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_start(handle: *mut c_void) -> AsimovCameraErrorCode {
    if handle.is_null() {
        return AsimovCameraErrorCode::InvalidArgument;
    }

    let handle = unsafe { &mut *(handle as *mut AsimovCameraHandle) };
    handle
        .driver
        .start()
        .map(|_| AsimovCameraErrorCode::Ok)
        .unwrap_or_else(Into::into)
}

/// Stop capturing frames.
///
/// Threading:
/// - May be called from any thread. Internally hops to main thread for
///   AVFoundation work.
///
/// Returns:
/// - `Ok` on success, or an appropriate error code.
///   Calling `stop` multiple times is safe.
#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_stop(handle: *mut c_void) -> AsimovCameraErrorCode {
    if handle.is_null() {
        return AsimovCameraErrorCode::InvalidArgument;
    }

    let handle = unsafe { &mut *(handle as *mut AsimovCameraHandle) };
    handle
        .driver
        .stop()
        .map(|_| AsimovCameraErrorCode::Ok)
        .unwrap_or_else(Into::into)
}

/// Free the camera handle and all associated resources.
///
/// After this call, the `handle` must not be used again.
///
/// Threading:
/// - Should be called from the main thread for consistency with how the
///   underlying AVFoundation objects are used, but the driver itself will
///   clean up safely even if the caller forgot to call `stop` first.
#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_free(handle: *mut c_void) {
    if handle.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(handle as *mut AsimovCameraHandle));
    }
}

/// Get the underlying `AVCaptureSession*` for iOS preview.
///
/// This returns the raw `AVCaptureSession*` used internally by the
/// AVFoundation-based driver, or NULL if no session is available.
///
/// Swift should:
/// - Cast this pointer to `AVCaptureSession`:
///   `let session = unsafeBitCast(ptr, to: AVCaptureSession.self)`
/// - Use it to create an `AVCaptureVideoPreviewLayer`.
///
/// Ownership / lifetime:
/// - The session is owned by Rust. Do **not** free or retain it directly
///   on the Swift side.
/// - It stays valid as long as the camera handle is alive, i.e. until
///   [`asimov_camera_free`] is called.
///
/// Threading:
/// - May be called from any thread. The access to `AVCaptureSession` is
///   performed on the main thread via `MainThreadBound::get_on_main`.
#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_get_session(handle: *mut c_void) -> *mut c_void {
    if handle.is_null() {
        return ptr::null_mut();
    }

    let handle = unsafe { &mut *(handle as *mut AsimovCameraHandle) };
    let driver = &handle.driver;

    let Some(bound) = driver.session_bound() else {
        return ptr::null_mut();
    };

    // `get_on_main` runs this closure on the main thread and returns its value.
    let result: SessionPtr = bound.get_on_main(|s: &Retained<AVCaptureSession>| {
        let ptr: *const AVCaptureSession = &**s;
        SessionPtr(ptr as *mut c_void)
    });

    result.0
}
