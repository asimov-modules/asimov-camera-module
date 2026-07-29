// This is free and unencumbered software released into the public domain.

#![allow(dead_code, unused_imports)]

use crate::drivers::CameraDriver;
use crate::{CameraBackend, CameraError, RawFrame, RawFrameRef};

use core::ffi::c_void;
use core::ptr::NonNull;

use dispatch2::{DispatchQueue, MainThreadBound};
use objc2::exception::catch;
use objc2::runtime::ProtocolObject;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, Message, define_class, msg_send, rc::Retained,
};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDeviceFormat,
    AVCaptureDeviceInput, AVCaptureDevicePosition, AVCaptureDeviceType,
    AVCaptureDeviceTypeBuiltInWideAngleCamera, AVCaptureDeviceTypeExternal, AVCaptureOutput,
    AVCaptureSession, AVCaptureVideoDataOutput, AVCaptureVideoDataOutputSampleBufferDelegate,
    AVMediaTypeVideo,
};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetDataSize,
    CVPixelBufferGetHeight, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{
    NSArray, NSDictionary, NSNumber, NSObject, NSObjectProtocol, NSString, ns_string,
};

use std::fmt;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_channel as ch;
use crossbeam_channel::{Receiver, Sender};

use crate::drivers::DriverConfig;

#[derive(Debug)]
struct NotMainThread;

impl fmt::Display for NotMainThread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AVFoundation must be initialized on the main thread")
    }
}

impl std::error::Error for NotMainThread {}

/// `MainThreadBound::get_on_main` requires the return value to be `Send`.
/// Raw pointers are not `Send`, so we wrap it in a newtype and mark it `Send`.
#[derive(Copy, Clone)]
struct SessionPtr(*mut c_void);
unsafe impl Send for SessionPtr {}

#[inline]
fn objc_catch_unit(context: &'static str, f: impl FnOnce()) -> Result<(), CameraError> {
    match catch(AssertUnwindSafe(f)) {
        Ok(()) => Ok(()),
        Err(_) => Err(CameraError::other(format!(
            "Objective-C exception while {context}"
        ))),
    }
}

#[inline]
fn objc_catch_value<T>(context: &'static str, f: impl FnOnce() -> T) -> Result<T, CameraError> {
    match catch(AssertUnwindSafe(f)) {
        Ok(v) => Ok(v),
        Err(_) => Err(CameraError::other(format!(
            "Objective-C exception while {context}"
        ))),
    }
}

pub struct AvfDriver {
    _cfg: DriverConfig,

    _raw_tx: Sender<RawFrameRef>,
    raw_rx: Receiver<RawFrameRef>,

    session: Option<MainThreadBound<Retained<AVCaptureSession>>>,
    delegate: Option<Retained<AvfCameraDelegate>>,
    _queue: Option<Retained<DispatchQueue>>,

    running: bool,
    closed: bool,
}

pub fn try_open(cfg: &DriverConfig) -> Result<Box<dyn CameraDriver>, CameraError> {
    AvfDriver::open(cfg).map(|d| Box::new(d) as Box<dyn CameraDriver>)
}

impl AvfDriver {
    pub fn open(cfg: &DriverConfig) -> Result<Self, CameraError> {
        let cfg = cfg.clone();
        cfg.validate()?;

        let cap = cfg.buffer_raw.max(1).min(32);
        let (raw_tx, raw_rx) = ch::bounded::<RawFrameRef>(cap);

        let mtm = MainThreadMarker::new()
            .ok_or_else(|| CameraError::driver("initializing AVFoundation", NotMainThread))?;

        let (session, delegate, queue) =
            objc_catch_value("creating/configuring AVCaptureSession", || unsafe {
                let session = AVCaptureSession::new();
                let (delegate, queue) = Self::configure_session(&session, &cfg, raw_tx.clone())?;
                Ok::<_, CameraError>((session, delegate, queue))
            })??;

        let session = MainThreadBound::new(session, mtm);

        Ok(Self {
            _cfg: cfg,
            _raw_tx: raw_tx,
            raw_rx,
            session: Some(session),
            delegate: Some(delegate),
            _queue: Some(queue),
            running: false,
            closed: false,
        })
    }

    unsafe fn configure_session(
        session: &AVCaptureSession,
        cfg: &DriverConfig,
        raw_tx: Sender<RawFrameRef>,
    ) -> Result<(Retained<AvfCameraDelegate>, Retained<DispatchQueue>), CameraError> {
        unsafe { session.beginConfiguration() };

        let result =
            (|| -> Result<(Retained<AvfCameraDelegate>, Retained<DispatchQueue>), CameraError> {
                let device = Self::find_device(cfg)?;

                let _ = objc_catch_unit("applying device format/fps configuration", || unsafe {
                    let _ = Self::apply_configuration_to_device(&device, cfg);
                });

                let input = unsafe { AVCaptureDeviceInput::deviceInputWithDevice_error(&device) }
                    .map_err(|err| {
                    // AVFoundationErrorDomain / AVErrorApplicationIsNotAuthorizedToUseDevice.
                    const AV_ERROR_NOT_AUTHORIZED: isize = -11852;
                    if err.code() == AV_ERROR_NOT_AUTHORIZED {
                        CameraError::PermissionDenied
                    } else {
                        CameraError::other("AVCaptureDeviceInput creation failed")
                    }
                })?;

                if unsafe { !session.canAddInput(&input) } {
                    return Err(CameraError::other("AVCaptureSession cannot add input"));
                }
                unsafe { session.addInput(&input) };

                let output = unsafe { AVCaptureVideoDataOutput::new() };

                {
                    let key = ns_string!("PixelFormatType");
                    let value = NSNumber::new_i32(i32::from_be_bytes(*b"BGRA"));
                    let settings = NSDictionary::<NSString>::from_slices(&[key], &[&value]);
                    unsafe { output.setVideoSettings(Some(&*settings)) };
                }

                unsafe { output.setAlwaysDiscardsLateVideoFrames(true) };

                let queue = DispatchQueue::new("asimov.camera.avf.queue", None);
                let queue: Retained<DispatchQueue> = queue.into();

                let delegate = AvfCameraDelegate::new(raw_tx);

                let protocol_obj = ProtocolObject::from_ref(&*delegate);
                unsafe { output.setSampleBufferDelegate_queue(Some(protocol_obj), Some(&*queue)) };

                if unsafe { !session.canAddOutput(&output) } {
                    return Err(CameraError::other("AVCaptureSession cannot add output"));
                }
                unsafe { session.addOutput(&output) };

                Ok((delegate, queue))
            })();

        unsafe { session.commitConfiguration() };
        result
    }

    fn find_device(cfg: &DriverConfig) -> Result<Retained<AVCaptureDevice>, CameraError> {
        let wanted_id = cfg.device.id();
        let wanted_name = cfg.device.name();

        if wanted_id.trim().is_empty() && wanted_name.trim().is_empty() {
            return unsafe {
                AVCaptureDevice::defaultDeviceWithMediaType(AVMediaTypeVideo.unwrap().as_ref())
            }
            .ok_or(CameraError::NoCamera);
        }

        let device_types: Retained<NSArray<AVCaptureDeviceType>> = unsafe {
            let built_in = AVCaptureDeviceTypeBuiltInWideAngleCamera;
            let external = AVCaptureDeviceTypeExternal;
            NSArray::from_slice(&[built_in, external])
        };

        let discovery: Retained<AVCaptureDeviceDiscoverySession> = unsafe {
            AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                &device_types,
                Some(AVMediaTypeVideo.expect("AVMediaTypeVideo is unavailable")),
                AVCaptureDevicePosition::Unspecified,
            )
        };

        let devices: Retained<NSArray<AVCaptureDevice>> = unsafe { discovery.devices() };

        if !wanted_id.trim().is_empty() {
            for dev in devices.iter() {
                let uid: Retained<NSString> = unsafe { dev.uniqueID() };
                if uid.to_string() == wanted_id {
                    return Ok(dev.retain());
                }
            }
        }

        if !wanted_name.trim().is_empty() {
            for dev in devices.iter() {
                let lname: Retained<NSString> = unsafe { dev.localizedName() };
                if lname.to_string() == wanted_name {
                    return Ok(dev.retain());
                }
            }
        }

        Err(CameraError::NoCamera)
    }

    unsafe fn apply_configuration_to_device(
        device: &AVCaptureDevice,
        cfg: &DriverConfig,
    ) -> Result<(), CameraError> {
        if cfg.width == 0 || cfg.height == 0 {
            return Ok(());
        }

        if unsafe { device.lockForConfiguration() }.is_err() {
            return Ok(());
        }

        let res = (|| -> Result<(), CameraError> {
            let formats = unsafe { device.formats() };

            // Prefer resolutions that can also hit the desired fps; fall back
            // to any resolution if none can, rather than silently doing
            // nothing (the old behavior when no *exact* size match existed).
            let mut fps_capable: Vec<(u32, u32)> = Vec::new();
            let mut all_dims: Vec<(u32, u32)> = Vec::new();

            for format in formats.iter() {
                let Some((w, h)) = format_dims(&format) else {
                    continue;
                };
                all_dims.push((w, h));
                if format_supports_fps(&format, cfg.fps) {
                    fps_capable.push((w, h));
                }
            }

            let candidates = if fps_capable.is_empty() {
                &all_dims
            } else {
                &fps_capable
            };

            let Some(best_dims) = crate::drivers::resolution::pick_nearest_resolution(
                (cfg.width, cfg.height),
                candidates,
            ) else {
                return Ok(());
            };

            // Among formats at the chosen resolution, prefer one that
            // supports the desired fps; otherwise take the first match.
            let mut chosen = None;
            for format in formats.iter() {
                if format_dims(&format) != Some(best_dims) {
                    continue;
                }
                if format_supports_fps(&format, cfg.fps) {
                    chosen = Some(format);
                    break;
                }
                if chosen.is_none() {
                    chosen = Some(format);
                }
            }

            if let Some(fmt) = chosen {
                unsafe { device.setActiveFormat(&fmt) };

                if cfg.fps.is_finite() && cfg.fps > 0.0 {
                    let fps_i32 = cfg.fps.round().max(1.0).min(i32::MAX as f64) as i32;
                    let duration = unsafe { CMTime::new(1, fps_i32) };
                    unsafe { device.setActiveVideoMinFrameDuration(duration) };
                    unsafe { device.setActiveVideoMaxFrameDuration(duration) };
                }
            }

            Ok(())
        })();

        unsafe { device.unlockForConfiguration() };
        res
    }

    fn teardown(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;

        let _ = self.stop();

        self.delegate = None;
        self._queue = None;
        self.session = None;
    }
}

impl Drop for AvfDriver {
    fn drop(&mut self) {
        self.teardown();
    }
}

impl CameraDriver for AvfDriver {
    fn backend(&self) -> CameraBackend {
        CameraBackend::Avf
    }

    fn start(&mut self) -> Result<(), CameraError> {
        if self.closed {
            return Err(CameraError::Closed);
        }
        if self.running {
            return Ok(());
        }

        let Some(ref session) = self.session else {
            return Err(CameraError::NotConfigured);
        };

        session.get_on_main(|s| {
            let _ = catch(AssertUnwindSafe(|| unsafe {
                s.startRunning();
            }));
        });

        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        if self.closed {
            return Ok(());
        }
        if !self.running {
            return Ok(());
        }

        if let Some(ref session) = self.session {
            session.get_on_main(|s| {
                let _ = catch(AssertUnwindSafe(|| unsafe {
                    s.stopRunning();
                }));
            });
        }

        self.running = false;
        Ok(())
    }

    fn close(&mut self) -> Result<(), CameraError> {
        self.teardown();
        Ok(())
    }

    fn read_frames(&mut self) -> Result<Receiver<RawFrameRef>, CameraError> {
        if self.closed {
            return Err(CameraError::Closed);
        }
        Ok(self.raw_rx.clone())
    }

    #[cfg(all(feature = "mobile-preview", feature = "avf", target_os = "ios"))]
    fn session_handle(&self) -> Result<crate::AvfSessionHandle, CameraError> {
        let Some(ref session) = self.session else {
            return Err(CameraError::NotConfigured);
        };

        let ptr = session
            .get_on_main(|s: &Retained<AVCaptureSession>| {
                let p: *const AVCaptureSession = &**s;
                SessionPtr(p as *mut c_void)
            })
            .0;

        let nn =
            NonNull::new(ptr).ok_or_else(|| CameraError::other("AVCaptureSession ptr is null"))?;
        Ok(unsafe { crate::AvfSessionHandle::from_nonnull_ptr(nn) })
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "AvfCameraDelegate"]
    #[ivars = AvfCameraDelegateVars]
    #[derive(Debug)]
    struct AvfCameraDelegate;

    unsafe impl NSObjectProtocol for AvfCameraDelegate {}

    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for AvfCameraDelegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        unsafe fn capture_output_did_output_sample_buffer_from_connection(
            &self,
            _capture_output: &AVCaptureOutput,
            sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            let _ = catch(AssertUnwindSafe(|| {
                let image_buffer = unsafe { CMSampleBuffer::image_buffer(sample_buffer) };
                let Some(pixel_buffer) = image_buffer else {
                    return;
                };

                if unsafe {
                    CVPixelBufferLockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly)
                } != 0
                {
                    return;
                }

                let did_lock = true;
                let _ = did_lock;

                let res = (|| {
                    let width = CVPixelBufferGetWidth(&pixel_buffer) as u32;
                    let height = CVPixelBufferGetHeight(&pixel_buffer) as u32;
                    let stride = CVPixelBufferGetBytesPerRow(&pixel_buffer) as u32;

                    let base = CVPixelBufferGetBaseAddress(&pixel_buffer);
                    let size = CVPixelBufferGetDataSize(&pixel_buffer);

                    if base.is_null() || size == 0 || width == 0 || height == 0 {
                        return;
                    }

                    let mut out = Vec::<u8>::with_capacity(size);
                    unsafe {
                        out.set_len(size);
                        core::ptr::copy_nonoverlapping(base as *const u8, out.as_mut_ptr(), size);
                    }

                    let ts = unsafe { CMSampleBuffer::presentation_time_stamp(sample_buffer) };
                    let timestamp_ns = cm_time_to_ns(ts);
                    let raw = RawFrame::new_bgra8(width, height, out, stride, Some(timestamp_ns));

                    let frame_ref: RawFrameRef = Arc::new(raw);

                    let vars = self.ivars();
                    let _ = vars.raw_tx.try_send(frame_ref);
                    vars.frame_counter.fetch_add(1, Ordering::Relaxed);
                })();

                unsafe {
                    CVPixelBufferUnlockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly)
                };
                res
            }));
        }
    }
);

impl AvfCameraDelegate {
    fn new(raw_tx: Sender<RawFrameRef>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(AvfCameraDelegateVars {
            raw_tx,
            frame_counter: AtomicU64::new(0),
        });
        unsafe { msg_send![super(this), init] }
    }
}

pub struct AvfCameraDelegateVars {
    pub raw_tx: Sender<RawFrameRef>,
    pub frame_counter: AtomicU64,
}

impl Clone for AvfCameraDelegateVars {
    fn clone(&self) -> Self {
        panic!("AvfCameraDelegateVars cannot be cloned");
    }
}

impl fmt::Debug for AvfCameraDelegateVars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AvfCameraDelegateVars {{ ... }}")
    }
}

fn cm_time_to_ns(t: CMTime) -> u64 {
    let ts = t.timescale;
    if ts <= 0 {
        return 0;
    }

    let value = t.value;
    if value <= 0 {
        return 0;
    }

    let value_u128 = value as u128;
    let ts_u128 = ts as u128;

    let ns = value_u128
        .saturating_mul(1_000_000_000u128)
        .saturating_div(ts_u128);

    ns.min(u64::MAX as u128) as u64
}

fn format_dims(format: &AVCaptureDeviceFormat) -> Option<(u32, u32)> {
    let desc = unsafe { format.formatDescription() };
    let dims = unsafe { objc2_core_media::CMVideoFormatDescriptionGetDimensions(&desc) };
    let (w, h) = (dims.width as u32, dims.height as u32);
    (w > 0 && h > 0).then_some((w, h))
}

fn format_supports_fps(format: &AVCaptureDeviceFormat, fps: f64) -> bool {
    if fps <= 0.0 {
        return true;
    }
    for range in unsafe { format.videoSupportedFrameRateRanges() } {
        if unsafe { range.maxFrameRate() } >= fps {
            return true;
        }
    }
    false
}
