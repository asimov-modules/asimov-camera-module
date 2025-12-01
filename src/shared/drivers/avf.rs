// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver, CameraError, FrameCallback};
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
            session.beginConfiguration();

            let device = Self::find_device(&config.device)?;

            Self::apply_configuration_to_device(&device, &config)?;

            let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&device)
                .map_err(|_| CameraError::DriverError)?;

            if session.canAddInput(&input) {
                session.addInput(&input);
            } else {
                return Err(CameraError::DriverError);
            }

            let output = AVCaptureVideoDataOutput::new();

            // BGRA Format
            let key = ns_string!("PixelFormatType");
            let value = NSNumber::new_i32(1111970369); // kCVPixelFormatType_32BGRA
            let settings = NSDictionary::<NSString>::from_slices(&[key], &[&value]);
            output.setVideoSettings(Some(&*settings));

            let queue = DispatchQueue::new("camera_queue", None);

            let delegate = AvfCameraDelegate::new(on_frame);
            let protocol_obj = ProtocolObject::from_ref(&*delegate);
            output.setSampleBufferDelegate_queue(Some(protocol_obj), Some(&*queue));

            if session.canAddOutput(&output) {
                session.addOutput(&output);
            } else {
                return Err(CameraError::DriverError);
            }

            session.commitConfiguration();

            Ok(Self {
                config,
                session: Some(session),
                delegate: Some(delegate),
            })
        }
    }

    /// Finds a device by Unique ID or Name. Falls back to default if config.device is empty.
    unsafe fn find_device(device_id: &str) -> Result<Retained<AVCaptureDevice>, CameraError> {
        if device_id.is_empty() {
            return AVCaptureDevice::defaultDeviceWithMediaType(AVMediaTypeVideo.unwrap().as_ref())
                .ok_or(CameraError::DriverError);
        }

        // Discovery Session: Look for Built-in and External (USB) cameras
        let device_types = NSArray::from_slice(&[
            AVCaptureDeviceTypeBuiltInWideAngleCamera.as_ref(),
            AVCaptureDeviceTypeExternal.as_ref(),
        ]);

        let discovery =
            AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                &device_types,
                AVMediaTypeVideo,
                AVCaptureDevicePosition::Unspecified,
            );

        let devices = discovery.devices();

        // 1. Try Exact Match on Unique ID
        for device in devices.iter() {
            let id = device.uniqueID();
            if id.to_string() == device_id {
                return Ok(device.retain());
            }
        }

        // 2. Try Match on Localized Name (User friendly name)
        for device in devices.iter() {
            let name = device.localizedName();
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

        // We must lock the device to change its active format
        // lockForConfiguration returns a BOOL in ObjC, or throws error.
        // Rust binding signatures vary, assuming standard Result or Bool return.
        // Using generic Result mapping here:
        if device.lockForConfiguration().is_err() {
            return Err(CameraError::NoCamera);
        }

        let formats = device.formats();
        let mut best_format = None;

        for format in formats.iter() {
            let desc = format.formatDescription();
            // Get dimensions from CMVideoFormatDescription
            let dimensions = objc2_core_media::CMVideoFormatDescriptionGetDimensions(&desc);

            if dimensions.width as u32 == config.width && dimensions.height as u32 == config.height
            {
                // Check FPS support
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

            // Set Frame Duration (inverse of FPS)
            // CMTimeMake(1, fps)
            let duration = CMTime::new(1, config.fps as i32);
            device.setActiveVideoMinFrameDuration(duration);
            device.setActiveVideoMaxFrameDuration(duration);
        }

        device.unlockForConfiguration();
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
            unsafe {
                let Some(pixel_buffer) = CMSampleBuffer::image_buffer(sample_buffer) else {
                    return;
                };

                if CVPixelBufferLockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly)
                    != 0
                {
                    return;
                }

                let width = CVPixelBufferGetWidth(&pixel_buffer);
                let height = CVPixelBufferGetHeight(&pixel_buffer);
                let bytes_per_row_in = CVPixelBufferGetBytesPerRow(&pixel_buffer);
                let base_address = CVPixelBufferGetBaseAddress(&pixel_buffer);

                // The input is BGRA (4 bytes per pixel)
                // We want RGB (3 bytes per pixel)
                let size_in = CVPixelBufferGetDataSize(&pixel_buffer);
                let data_in = core::slice::from_raw_parts(base_address as *const u8, size_in);

                let output_len = (width * height * 3) as usize; // 3 bytes per pixel
                let mut data_out = Vec::with_capacity(output_len);

                let width_usize = width as usize;
                let height_usize = height as usize;

                // Iterate row by row (necessary due to bytes_per_row padding)
                for y in 0..height_usize {
                    let row_start_in = y * bytes_per_row_in;
                    let row_end_in = row_start_in + width_usize * 4; // 4 bytes per pixel input

                    if row_end_in > size_in {
                        continue;
                    }

                    // Process 4 bytes (BGRA) -> 3 bytes (RGB)
                    for x in (row_start_in..row_end_in).step_by(4) {
                        // Input format is BGRA (B=x, G=x+1, R=x+2, A=x+3)

                        let r = data_in[x + 2]; // R
                        let g = data_in[x + 1]; // G
                        let b = data_in[x + 0]; // B
                        // A is ignored (data_in[x + 3])

                        data_out.push(r);
                        data_out.push(g);
                        data_out.push(b);
                    }
                }

                // Note: We use the *new* RGB buffer size and 3 bytes per pixel for output
                (self.ivars().callback)(data_out.as_slice(), width, height, width * 3);

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
