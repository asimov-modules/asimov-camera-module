// This is free and unencumbered software released into the public domain.

//! AVFoundation-based camera driver for iOS/macOS.
//!
//! This driver:
//! - Opens and configures an `AVCaptureSession` based on [`CameraConfig`].
//! - Captures BGRA frames and forwards them to a [`FrameCallback`] on a
//!   dedicated `DispatchQueue`.
//! - Exposes the underlying `AVCaptureSession` via [`AvfCameraDriver::session_bound`]
//!   so that the iOS FFI layer can create a live preview (`AVCaptureVideoPreviewLayer`).
//!
//! Threading model:
//! - [`AvfCameraDriver::open`] **must** be called on the main thread.
//!   If it is not, it returns [`CameraError::DriverError`].
//! - [`CameraDriver::start`] and [`CameraDriver::stop`] *may* be called from any
//!   thread. Internally, AVFoundation calls are always executed on the main thread
//!   via `MainThreadBound`.
//!
//! Ownership & lifetime:
//! - `AvfCameraDriver` owns the `AVCaptureSession` and the delegate.
//! - Resources are released when [`CameraDriver::stop`] is called or when the
//!   driver is dropped. [`Drop`] calls `stop()` as a best-effort safety net.

use crate::shared::{CameraConfig, CameraDriver, CameraError, Frame, FrameCallback};
use alloc::borrow::Cow;
use dispatch2::{DispatchQueue, MainThreadBound};
use objc2::runtime::ProtocolObject;
use objc2::{
    AllocAnyThread, DeclaredClass, MainThreadMarker, Message, define_class, msg_send, rc::Retained,
};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDeviceInput,
    AVCaptureDevicePosition, AVCaptureDeviceTypeBuiltInWideAngleCamera,
    AVCaptureDeviceTypeExternal, AVCaptureOutput, AVCaptureSession, AVCaptureVideoDataOutput,
    AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaTypeVideo,
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

/// AVFoundation-backed camera driver.
///
/// Created with [`AvfCameraDriver::open`], then controlled through the
/// [`CameraDriver`] trait (`start` / `stop`).
///
/// On iOS this is wrapped by the FFI layer (`src/ffi.rs`) and used from Swift.
#[derive(Debug)]
pub struct AvfCameraDriver {
    /// Configuration used to initialize the driver.
    pub config: CameraConfig,
    /// Main-thread-bound `AVCaptureSession`.
    ///
    /// We wrap the session in `MainThreadBound` so that the driver struct
    /// itself can be moved across threads while the actual AVFoundation work
    /// still happens on the main thread.
    session: Option<MainThreadBound<Retained<AVCaptureSession>>>,
    /// Strong reference to the delegate that receives video frames.
    ///
    /// This must be kept alive as long as the session is running; otherwise
    /// AVFoundation will stop delivering frames.
    #[allow(unused)]
    delegate: Option<Retained<AvfCameraDelegate>>,
}

impl dogma::Named for AvfCameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "avf".into()
    }
}

impl AvfCameraDriver {
    /// Create a new AVFoundation camera driver.
    ///
    /// - `config.device` may be empty to select the default video device;
    ///   otherwise it is matched against device unique ID or localized name.
    /// - `config.width` / `height` / `fps` are best-effort; if the exact
    ///   combination is not supported, the device format is left unchanged.
    /// - `callback` is invoked on a dedicated background `DispatchQueue` for
    ///   each frame, with BGRA pixel data.
    ///
    /// # Threading
    ///
    /// This must be called on the **main thread**. If called from a non-main
    /// thread, it returns [`CameraError::DriverError`].
    pub fn open(
        _input_url: impl AsRef<str>,
        config: CameraConfig,
        callback: FrameCallback,
    ) -> Result<Self, CameraError> {
        unsafe {
            // Ensure we are running on the main thread before touching AVFoundation.
            let mtm = MainThreadMarker::new().ok_or(CameraError::DriverError)?;

            let session = AVCaptureSession::new();
            let delegate = Self::configure_session(&session, &config, callback)?;

            let session = MainThreadBound::new(session, mtm);

            Ok(Self {
                config,
                session: Some(session),
                delegate: Some(delegate),
            })
        }
    }

    /// Return the bound `AVCaptureSession`.
    ///
    /// This is used by the iOS FFI layer to obtain a raw `AVCaptureSession*`
    /// and construct an `AVCaptureVideoPreviewLayer` for live preview.
    pub fn session_bound(&self) -> Option<&MainThreadBound<Retained<AVCaptureSession>>> {
        self.session.as_ref()
    }

    /// Configures an `AVCaptureSession` for video capture.
    ///
    /// This helper:
    /// - starts a configuration transaction (`beginConfiguration` / `commitConfiguration`)
    /// - selects the capture device based on `CameraConfig::device`
    /// - applies the requested width / height / FPS if supported
    /// - creates and attaches an `AVCaptureDeviceInput`
    /// - creates a `AVCaptureVideoDataOutput` configured for BGRA frames
    /// - installs an `AvfCameraDelegate` wired to the provided `FrameCallback`
    ///
    /// On success returns the retained delegate associated with the session.
    /// On error returns `CameraError`, and the session is still left in a
    /// committed, consistent state (no dangling configuration transaction).
    fn configure_session(
        session: &AVCaptureSession,
        config: &CameraConfig,
        callback: FrameCallback,
    ) -> Result<Retained<AvfCameraDelegate>, CameraError> {
        unsafe { session.beginConfiguration() };

        let result: Result<Retained<AvfCameraDelegate>, CameraError> = (|| {
            let device = Self::find_device(&config.device)?;

            unsafe {
                Self::apply_configuration_to_device(&device, config)?;
            }

            let input = unsafe { AVCaptureDeviceInput::deviceInputWithDevice_error(&device) }
                .map_err(|_| CameraError::DriverError)?;

            unsafe {
                if !session.canAddInput(&input) {
                    return Err(CameraError::DriverError);
                }
                session.addInput(&input);
            }

            let output = unsafe { AVCaptureVideoDataOutput::new() };

            // BGRA pixel format ("BGRA" → kCVPixelFormatType_32BGRA).
            unsafe {
                let key = ns_string!("PixelFormatType");
                let value = NSNumber::new_i32(i32::from_be_bytes(*b"BGRA"));
                let settings = NSDictionary::<NSString>::from_slices(&[key], &[&value]);
                output.setVideoSettings(Some(&*settings));
            }

            let queue = DispatchQueue::new("asimov.camera.avf.queue", None);
            let delegate = AvfCameraDelegate::new(callback);

            unsafe {
                let protocol_obj = ProtocolObject::from_ref(&*delegate);
                output.setSampleBufferDelegate_queue(Some(protocol_obj), Some(&*queue));

                if !session.canAddOutput(&output) {
                    return Err(CameraError::DriverError);
                }
                session.addOutput(&output);
            }

            Ok(delegate)
        })();

        unsafe { session.commitConfiguration() };

        result
    }

    /// Finds a device by unique ID or localized name.
    ///
    /// - If `device_id` is empty, returns the default video device (if any).
    /// - Otherwise tries to match `uniqueID`, then `localizedName`.
    fn find_device(device_id: &str) -> Result<Retained<AVCaptureDevice>, CameraError> {
        if device_id.is_empty() {
            return unsafe {
                AVCaptureDevice::defaultDeviceWithMediaType(AVMediaTypeVideo.unwrap().as_ref())
            }
            .ok_or(CameraError::DriverError);
        }

        // Discovery Session: Look for built-in and external (USB) cameras.
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

    /// Iterates supported formats to find one matching width/height/fps.
    ///
    /// This is best-effort. If no matching format is found, the device's
    /// active format is left unchanged.
    unsafe fn apply_configuration_to_device(
        device: &AVCaptureDevice,
        config: &CameraConfig,
    ) -> Result<(), CameraError> {
        if config.width == 0 || config.height == 0 {
            return Ok(());
        }

        if unsafe { device.lockForConfiguration() }.is_err() {
            return Err(CameraError::NoCamera);
        }

        let formats = unsafe { device.formats() };
        let mut best_format = None;

        for format in formats.iter() {
            let desc = unsafe { format.formatDescription() };
            let dimensions =
                unsafe { objc2_core_media::CMVideoFormatDescriptionGetDimensions(&desc) };

            if dimensions.width as u32 == config.width && dimensions.height as u32 == config.height
            {
                for range in unsafe { format.videoSupportedFrameRateRanges() } {
                    let max_rate = unsafe { range.maxFrameRate() };
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
            unsafe {
                device.setActiveFormat(&fmt);

                let duration = CMTime::new(1, config.fps as i32);
                device.setActiveVideoMinFrameDuration(duration);
                device.setActiveVideoMaxFrameDuration(duration);
            }
        }

        unsafe { device.unlockForConfiguration() };
        Ok(())
    }
}

impl CameraDriver for AvfCameraDriver {
    /// Start the `AVCaptureSession`.
    ///
    /// Threading:
    /// - May be called from any thread; internally this hops to the main
    ///   thread via `MainThreadBound::get_on_main`.
    fn start(&mut self) -> Result<(), CameraError> {
        let Some(ref session) = self.session else {
            return Err(CameraError::NoCamera);
        };

        session.get_on_main(|s| unsafe {
            s.startRunning();
        });

        Ok(())
    }

    /// Stop the `AVCaptureSession` and release resources.
    ///
    /// Threading:
    /// - May be called from any thread; internally this hops to the main
    ///   thread via `MainThreadBound::get_on_main`.
    fn stop(&mut self) -> Result<(), CameraError> {
        let Some(ref session) = self.session else {
            return Ok(());
        };

        session.get_on_main(|s| unsafe {
            s.stopRunning();
        });

        Ok(())
    }
}

impl Drop for AvfCameraDriver {
    /// Best-effort cleanup if the caller forgot to call `stop()`.
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
            let Some(pixel_buffer) = (unsafe { CMSampleBuffer::image_buffer(sample_buffer) })
            else {
                return;
            };

            if unsafe {
                CVPixelBufferLockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly)
            } != 0
            {
                return;
            }

            let width = CVPixelBufferGetWidth(&pixel_buffer);
            let height = CVPixelBufferGetHeight(&pixel_buffer);
            let bytes_per_row = CVPixelBufferGetBytesPerRow(&pixel_buffer);
            let base_address = CVPixelBufferGetBaseAddress(&pixel_buffer);
            let size = CVPixelBufferGetDataSize(&pixel_buffer);

            if base_address.is_null() || size == 0 {
                unsafe {
                    CVPixelBufferUnlockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly);
                }
                return;
            }

            let data_in = unsafe { core::slice::from_raw_parts(base_address as *const u8, size) };
            let data = data_in.to_vec();

            let frame = Frame::new_bgra(data, width, height, bytes_per_row);
            (self.ivars().callback)(frame);

            unsafe {
                CVPixelBufferUnlockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly);
            }
        }
    }
);

impl AvfCameraDelegate {
    /// Create a new delegate that forwards frames to the given callback.
    fn new(callback: FrameCallback) -> Retained<Self> {
        let this = Self::alloc().set_ivars(AvfCameraDelegateVars { callback });
        unsafe { msg_send![super(this), init] }
    }
}

/// Stored ivars for `AvfCameraDelegate`.
///
/// We intentionally do not implement `Clone` because `FrameCallback`
/// (`Box<dyn Fn(Frame)...>`) is not clonable.
pub struct AvfCameraDelegateVars {
    pub(crate) callback: FrameCallback,
}

impl Clone for AvfCameraDelegateVars {
    fn clone(&self) -> Self {
        // Since FrameCallback (Box<dyn Fn...>) cannot be cloned,
        // cloning the delegate vars is a logic error.
        panic!("AvfCameraDelegateVars cannot be cloned.");
    }
}

impl core::fmt::Debug for AvfCameraDelegateVars {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AvfCameraDelegateVars {{ callback: ... }}")
    }
}
