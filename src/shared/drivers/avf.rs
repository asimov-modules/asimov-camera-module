// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver, CameraError, FrameCallback};
use alloc::borrow::Cow;
use objc2::{AllocAnyThread, define_class, msg_send, rc::Retained};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureOutput, AVCaptureSession, AVCaptureVideoDataOutput,
    AVCaptureVideoDataOutputSampleBufferDelegate,
};
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetDataSize,
    CVPixelBufferGetHeight, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
};
use objc2_foundation::{NSObject, NSObjectProtocol};

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
        _callback: FrameCallback,
    ) -> Result<Self, CameraError> {
        unsafe {
            let session = AVCaptureSession::new();
            let _output = AVCaptureVideoDataOutput::new();
            let delegate = AvfCameraDelegate::new();
            session.beginConfiguration();
            session.commitConfiguration();
            Ok(Self {
                config,
                session: Some(session),
                delegate: Some(delegate),
            })
        }
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
                CVPixelBufferLockBaseAddress(&pixel_buffer, CVPixelBufferLockFlags::ReadOnly);
                let _format_type = CVPixelBufferGetPixelFormatType(&pixel_buffer);
                let _width = CVPixelBufferGetWidth(&pixel_buffer);
                let _height = CVPixelBufferGetHeight(&pixel_buffer);
                let _bytes_per_row = CVPixelBufferGetBytesPerRow(&pixel_buffer);
                let base_address = CVPixelBufferGetBaseAddress(&pixel_buffer);
                let size = CVPixelBufferGetDataSize(&pixel_buffer);
                let _data = core::slice::from_raw_parts(base_address as *mut u8, size);
                // TODO
            }
            todo!()
        }
    }
);

impl AvfCameraDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(AvfCameraDelegateVars {});
        unsafe { msg_send![super(this), init] }
    }
}

unsafe impl NSObjectProtocol for AvfCameraDelegate {}

#[derive(Clone, Debug)]
pub struct AvfCameraDelegateVars {}
