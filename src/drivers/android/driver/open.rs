// This is free and unencumbered software released into the public domain.

use std::ffi::{CString, c_char, c_void};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crossbeam_channel as ch;
use ndk_sys as ndk;

use crate::drivers::DriverConfig;
use crate::{CameraError, RawFrameRef};

use super::callbacks;
use super::image_stream::ImageStream;
use super::session_graph::SessionGraph;
use super::sizes;

use crate::drivers::android::ndk::{CameraManager, NativeWindow};

use super::AndroidDriver;

pub fn open(cfg: &DriverConfig) -> Result<AndroidDriver, CameraError> {
    let cfg = cfg.clone();
    cfg.validate()?;

    let id = cfg.device.id().trim();
    if id.is_empty() {
        return Err(CameraError::invalid_config("Android camera id is empty"));
    }

    let cap = cfg.buffer_raw.max(1).min(8);
    let (raw_tx, raw_rx) = ch::bounded::<RawFrameRef>(cap);

    let preview_ptr: *mut ndk::ANativeWindow =
        cfg.android_preview.as_ptr().cast::<ndk::ANativeWindow>();

    if preview_ptr.is_null() {
        return Err(CameraError::invalid_config(
            "android_preview must be a non-null ANativeWindow pointer",
        ));
    }

    let mut preview_window = NativeWindow::from_ptr_borrowed(preview_ptr);
    preview_window.acquire();
    let pw = preview_window.width();
    let ph = preview_window.height();

    let mgr = CameraManager::new()
        .map_err(|st| CameraError::driver("android: ACameraManager_create", st))?;

    let camera_id =
        CString::new(id).map_err(|_| CameraError::invalid_config("device id contains NUL byte"))?;

    const AIMAGE_FORMAT_YUV_420_888: i32 = 35;

    let desired_w: i32 = if pw > 0 { pw } else { cfg.width as i32 };
    let desired_h: i32 = if ph > 0 { ph } else { cfg.height as i32 };

    let supported = sizes::list_supported_output_sizes(
        &mgr,
        camera_id.as_ptr() as *const c_char,
        AIMAGE_FORMAT_YUV_420_888,
    );

    let (best_w, best_h) =
        sizes::pick_best_size(desired_w, desired_h, &supported).unwrap_or((1280, 720));

    let preview_fmt = preview_window.format();
    let _ = preview_window.set_buffers_geometry(best_w, best_h, preview_fmt);

    let state = Arc::new(callbacks::State::new(raw_tx.clone()));
    let state_ptr = callbacks::state_ptr(&state) as *mut c_void;

    let device_callbacks = callbacks::build_device_callbacks(state_ptr);

    let dev = mgr
        .open_camera(camera_id.as_ptr() as *const c_char, device_callbacks)
        .map_err(|st| CameraError::driver("android: ACameraManager_openCamera", st))?;

    let max_images: i32 = (cfg.buffer_raw.max(2).min(4)) as i32;

    let image_stream = ImageStream::new(
        (best_w as u32, best_h as u32),
        AIMAGE_FORMAT_YUV_420_888,
        max_images,
        state_ptr,
    )
    .map_err(|st| CameraError::driver("android: ImageStream::new", st))?;

    let session_callbacks = callbacks::build_session_callbacks(state_ptr);

    let graph = SessionGraph::new(
        &dev,
        &mut preview_window,
        image_stream.window_ptr(),
        session_callbacks,
    )?;

    let rotation_deg: i32 = 90;

    Ok(AndroidDriver {
        _cfg: cfg,
        _raw_tx: raw_tx,
        raw_rx,

        _mgr: mgr,
        _dev: dev,

        preview_window,
        image_stream,
        graph,
        running: AtomicBool::new(false),
        closed: AtomicBool::new(false),
        _state: state,

        picked_w: best_w.max(0) as u32,
        picked_h: best_h.max(0) as u32,
        rotation_deg,
    })
}
