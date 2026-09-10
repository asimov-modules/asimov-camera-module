// This is free and unencumbered software released into the public domain.

use core::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crossbeam_channel as ch;
use ndk_sys as ndk;

use crate::RawFrameRef;

pub struct State {
    pub last_device_error: AtomicI32,
    pub disconnected: AtomicBool,
    pub raw_tx: ch::Sender<RawFrameRef>,
}

impl State {
    pub fn new(raw_tx: ch::Sender<RawFrameRef>) -> Self {
        Self {
            last_device_error: AtomicI32::new(0),
            disconnected: AtomicBool::new(false),
            raw_tx,
        }
    }
}

#[inline]
pub fn state_ptr(st: &Arc<State>) -> *mut c_void {
    Arc::as_ptr(st) as *mut c_void
}

extern "C" fn on_disconnected(ctx: *mut c_void, _device: *mut ndk::ACameraDevice) {
    if ctx.is_null() {
        return;
    }
    let st = unsafe { &*(ctx as *const State) };
    st.disconnected.store(true, Ordering::Release);
}

extern "C" fn on_error(ctx: *mut c_void, _device: *mut ndk::ACameraDevice, error_code: i32) {
    if ctx.is_null() {
        return;
    }
    let st = unsafe { &*(ctx as *const State) };
    st.last_device_error.store(error_code, Ordering::Release);
}

pub fn build_device_callbacks(state_ptr: *mut c_void) -> Box<ndk::ACameraDevice_StateCallbacks> {
    Box::new(ndk::ACameraDevice_StateCallbacks {
        context: state_ptr,
        onDisconnected: Some(on_disconnected),
        onError: Some(on_error),
    })
}

extern "C" fn on_session_ready(ctx: *mut c_void, _session: *mut ndk::ACameraCaptureSession) {
    if ctx.is_null() {
        return;
    }
    let st = unsafe { &*(ctx as *const State) };
    let _ = st.last_device_error.load(Ordering::Acquire);
}

extern "C" fn on_session_active(ctx: *mut c_void, _session: *mut ndk::ACameraCaptureSession) {
    if ctx.is_null() {
        return;
    }
    let st = unsafe { &*(ctx as *const State) };
    let _ = st.last_device_error.load(Ordering::Acquire);
}

extern "C" fn on_session_closed(_ctx: *mut c_void, _session: *mut ndk::ACameraCaptureSession) {}

pub fn build_session_callbacks(
    state_ptr: *mut c_void,
) -> Box<ndk::ACameraCaptureSession_stateCallbacks> {
    Box::new(ndk::ACameraCaptureSession_stateCallbacks {
        context: state_ptr,
        onClosed: Some(on_session_closed),
        onReady: Some(on_session_ready),
        onActive: Some(on_session_active),
    })
}
