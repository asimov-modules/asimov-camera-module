// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver, CameraError, Frame, FrameCallback};
use alloc::borrow::Cow;
use dispatch2::DispatchQueue;
use objc2::runtime::ProtocolObject;
use objc2::{AllocAnyThread, DeclaredClass, Message, define_class, msg_send, rc::Retained};
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

#[derive(Clone, Debug)]
pub struct AvfCameraDriver {
    pub config: CameraConfig,
    session: Option<Retained<AVCaptureSession>>,
    #[allow(unused)]
    delegate: Option<Retained<AvfCameraDelegate>>,
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
        on_frame: FrameCallback,
    ) -> Result<Self, CameraError> {
        unsafe {
            let session = AVCaptureSession::new();
            let delegate = Self::configure_session(&session, &config, on_frame)?;

            Ok(Self {
                config,
                session: Some(session),
                delegate: Some(delegate),
            })
        }
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

    /// Finds a device by Unique ID or Name. Falls back to default if config.device is empty.
    fn find_device(device_id: &str) -> Result<Retained<AVCaptureDevice>, CameraError> {
        if device_id.is_empty() {
            return unsafe {
                AVCaptureDevice::defaultDeviceWithMediaType(AVMediaTypeVideo.unwrap().as_ref())
            }
            .ok_or(CameraError::DriverError);
        }

        // Discovery Session: Look for Built-in and External (USB) cameras
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

    /// Iterates supported formats to find one matching width/height/fps
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
            unsafe { device.setActiveFormat(&fmt) };

            let duration = unsafe { CMTime::new(1, config.fps as i32) };
            unsafe {
                device.setActiveVideoMinFrameDuration(duration);
                device.setActiveVideoMaxFrameDuration(duration);
            }
        }

        unsafe { device.unlockForConfiguration() };
        Ok(())
    }
}

impl CameraDriver for AvfCameraDriver {
    fn start(&mut self) -> Result<(), CameraError> {
        let Some(ref session) = self.session else {
            return Err(CameraError::NoCamera); // TODO
        };
        unsafe {
            session.startRunning();
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        let Some(ref session) = self.session else {
            return Ok(());
        };
        unsafe {
            session.stopRunning();
        }
        self.session = None;
        self.delegate = None;
        Ok(())
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "AvfCameraDelegate"]
    #[ivars = AvfCameraDelegateVars]
    #[derive(Debug)]
    struct AvfCameraDelegate;

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
    fn new(callback: FrameCallback) -> Retained<Self> {
        let this = Self::alloc().set_ivars(AvfCameraDelegateVars { callback });
        unsafe { msg_send![super(this), init] }
    }
}

unsafe impl NSObjectProtocol for AvfCameraDelegate {}

pub struct AvfCameraDelegateVars {
    callback: FrameCallback,
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
