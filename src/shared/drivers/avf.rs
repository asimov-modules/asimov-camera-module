// This is free and unencumbered software released into the public domain.

use crate::shared::{
    try_send_frame, CameraBackend, CameraConfig, CameraDriver, CameraError, CameraEvent, Frame,
    FrameMsg, PixelFormat,
};
use alloc::borrow::Cow;
use bytes::Bytes;
use dispatch2::{DispatchQueue, MainThreadBound};
use objc2::runtime::ProtocolObject;
use objc2::{
    define_class, msg_send, rc::Retained, AllocAnyThread, DeclaredClass, MainThreadMarker, Message,
};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDeviceInput,
    AVCaptureDevicePosition, AVCaptureDeviceTypeBuiltInWideAngleCamera, AVCaptureDeviceTypeExternal,
    AVCaptureOutput, AVCaptureSession, AVCaptureVideoDataOutput,
    AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaTypeVideo,
};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetDataSize,
    CVPixelBufferGetHeight, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSObject, NSObjectProtocol, NSString, ns_string};
use std::{any::Any, error::Error as StdError, fmt, sync::mpsc::SyncSender};

#[derive(Debug)]
struct NotMainThread;

impl fmt::Display for NotMainThread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AVFoundation must be initialized on the main thread")
    }
}

impl StdError for NotMainThread {}

#[derive(Debug)]
pub struct AvfCameraDriver {
    config: CameraConfig,
    backend: CameraBackend,
    session: Option<MainThreadBound<Retained<AVCaptureSession>>>,
    delegate: Option<Retained<AvfCameraDelegate>>,
    frame_tx: SyncSender<FrameMsg>,
    events_tx: SyncSender<CameraEvent>,
}

impl dogma::Named for AvfCameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "avf".into()
    }
}

impl AvfCameraDriver {
    pub fn open(
        _input_url: impl AsRef<str>,
        config: CameraConfig,
        frame_tx: SyncSender<FrameMsg>,
        events_tx: SyncSender<CameraEvent>,
    ) -> Result<Self, CameraError> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| CameraError::driver("initializing AVFoundation", NotMainThread))?;

        unsafe {
            let session = AVCaptureSession::new();
            let delegate = Self::configure_session(&session, &config, &frame_tx, &events_tx)?;

            let session = MainThreadBound::new(session, mtm);

            Ok(Self {
                config,
                backend: CameraBackend::Avf,
                session: Some(session),
                delegate: Some(delegate),
                frame_tx,
                events_tx,
            })
        }
    }

    pub fn session_bound(&self) -> Option<&MainThreadBound<Retained<AVCaptureSession>>> {
        self.session.as_ref()
    }

    unsafe fn configure_session(
        session: &AVCaptureSession,
        config: &CameraConfig,
        frame_tx: &SyncSender<FrameMsg>,
        events_tx: &SyncSender<CameraEvent>,
    ) -> Result<Retained<AvfCameraDelegate>, CameraError> {
        session.beginConfiguration();

        let result: Result<Retained<AvfCameraDelegate>, CameraError> = (|| {
            let device = Self::find_device(config.device.as_deref().unwrap_or(""))?;

            Self::apply_configuration_to_device(&device, config)?;

            let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&device)
                .map_err(|_| CameraError::other("AVCaptureDeviceInput creation failed"))?;

            if !session.canAddInput(&input) {
                return Err(CameraError::other("AVCaptureSession cannot add input"));
            }
            session.addInput(&input);

            let output = AVCaptureVideoDataOutput::new();

            {
                let key = ns_string!("PixelFormatType");
                let value = NSNumber::new_i32(i32::from_be_bytes(*b"BGRA"));
                let settings = NSDictionary::<NSString>::from_slices(&[key], &[&value]);
                output.setVideoSettings(Some(&*settings));
            }

            output.setAlwaysDiscardsLateVideoFrames(true);

            let queue = DispatchQueue::new("asimov.camera.avf.queue", None);
            let delegate = AvfCameraDelegate::new(
                frame_tx.clone(),
                events_tx.clone(),
                CameraBackend::Avf,
            );

            let protocol_obj = ProtocolObject::from_ref(&*delegate);
            output.setSampleBufferDelegate_queue(Some(protocol_obj), Some(&*queue));

            if !session.canAddOutput(&output) {
                return Err(CameraError::other("AVCaptureSession cannot add output"));
            }
            session.addOutput(&output);

            Ok(delegate)
        })();

        session.commitConfiguration();

        result
    }

    fn find_device(device_id: &str) -> Result<Retained<AVCaptureDevice>, CameraError> {
        if device_id.is_empty() {
            return unsafe {
                AVCaptureDevice::defaultDeviceWithMediaType(AVMediaTypeVideo.unwrap().as_ref())
            }
                .ok_or(CameraError::NoCamera);
        }

        let device_types = unsafe {
            NSArray::from_slice(&[
                AVCaptureDeviceTypeBuiltInWideAngleCamera.as_ref(),
                AVCaptureDeviceTypeExternal.as_ref(),
            ])
        };

        let discovery = unsafe {
            AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                &device_types,
                AVMediaTypeVideo,
                AVCaptureDevicePosition::Unspecified,
            )
        };

        let devices = unsafe { discovery.devices() };

        for device in devices.iter() {
            let id = unsafe { device.uniqueID() };
            if id.to_string() == device_id {
                return Ok(device.retain());
            }
        }

        for device in devices.iter() {
            let name = unsafe { device.localizedName() };
            if name.to_string() == device_id {
                return Ok(device.retain());
            }
        }

        Err(CameraError::NoCamera)
    }

    unsafe fn apply_configuration_to_device(
        device: &AVCaptureDevice,
        config: &CameraConfig,
    ) -> Result<(), CameraError> {
        if config.width == 0 || config.height == 0 {
            return Ok(());
        }

        if device.lockForConfiguration().is_err() {
            return Err(CameraError::NoCamera);
        }

        let formats = device.formats();
        let mut best_format = None;

        for format in formats.iter() {
            let desc = format.formatDescription();
            let dims = objc2_core_media::CMVideoFormatDescriptionGetDimensions(&desc);

            if dims.width as u32 == config.width && dims.height as u32 == config.height {
                for range in format.videoSupportedFrameRateRanges() {
                    let max_rate = range.maxFrameRate();
                    if max_rate >= config.fps {
                        best_format = Some(format);
                        break;
                    }
                }
            }
            if best_format.is_some() {
                break;
            }
        }

        if let Some(fmt) = best_format {
            device.setActiveFormat(&fmt);

            if config.fps > 0.0 {
                let fps_i32 = config.fps.round().max(1.0).min(i32::MAX as f64) as i32;
                let duration = CMTime::new(1, fps_i32);
                device.setActiveVideoMinFrameDuration(duration);
                device.setActiveVideoMaxFrameDuration(duration);
            }
        }

        device.unlockForConfiguration();
        Ok(())
    }
}

impl CameraDriver for AvfCameraDriver {
    fn backend(&self) -> CameraBackend {
        self.backend
    }

    fn start(&mut self) -> Result<(), CameraError> {
        let Some(ref session) = self.session else {
            return Err(CameraError::NoCamera);
        };

        session.get_on_main(|s| unsafe {
            s.startRunning();
        });

        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        let Some(ref session) = self.session else {
            return Ok(());
        };

        session.get_on_main(|s| unsafe {
            s.stopRunning();
        });

        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Drop for AvfCameraDriver {
    fn drop(&mut self) {
        let _ = self.stop();
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
            let Some(pixel_buffer) = CMSampleBuffer::image_buffer(sample_buffer) else {
                return;
            };

            if CVPixelBufferLockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly) != 0 {
                return;
            }

            let width = CVPixelBufferGetWidth(&pixel_buffer) as u32;
            let height = CVPixelBufferGetHeight(&pixel_buffer) as u32;
            let stride = CVPixelBufferGetBytesPerRow(&pixel_buffer) as u32;
            let base = CVPixelBufferGetBaseAddress(&pixel_buffer);
            let size = CVPixelBufferGetDataSize(&pixel_buffer);

            if base.is_null() || size == 0 {
                CVPixelBufferUnlockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly);
                return;
            }

            let data_in = core::slice::from_raw_parts(base as *const u8, size);
            let data = Bytes::copy_from_slice(data_in);

            let ts = CMSampleBuffer::presentation_time_stamp(sample_buffer);
            let timestamp_ns = cm_time_to_ns(ts);

            let frame = Frame::new(data, width, height, stride, PixelFormat::Bgra8)
                .with_timestamp_ns(timestamp_ns);

            let vars = self.ivars();
            try_send_frame(&vars.frame_tx, &vars.events_tx, vars.backend, frame);

            CVPixelBufferUnlockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly);
        }
    }
);

impl AvfCameraDelegate {
    fn new(
        frame_tx: SyncSender<FrameMsg>,
        events_tx: SyncSender<CameraEvent>,
        backend: CameraBackend,
    ) -> Retained<Self> {
        let this = Self::alloc().set_ivars(AvfCameraDelegateVars {
            frame_tx,
            events_tx,
            backend,
        });
        unsafe { msg_send![super(this), init] }
    }
}

pub struct AvfCameraDelegateVars {
    pub(crate) frame_tx: SyncSender<FrameMsg>,
    pub(crate) events_tx: SyncSender<CameraEvent>,
    pub(crate) backend: CameraBackend,
}

impl Clone for AvfCameraDelegateVars {
    fn clone(&self) -> Self {
        panic!("AvfCameraDelegateVars cannot be cloned");
    }
}

impl core::fmt::Debug for AvfCameraDelegateVars {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AvfCameraDelegateVars {{ ... }}")
    }
}

fn cm_time_to_ns(t: CMTime) -> u64 {
    // In objc2_core_media, CMTime exposes fields (value, timescale), not methods.
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
