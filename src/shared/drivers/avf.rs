// This is free and unencumbered software released into the public domain.

use crate::shared::{
    CameraBackend, CameraConfig, CameraDriver, CameraError, CameraEvent, FrameMsg,
};
use alloc::borrow::Cow;
use std::{any::Any, sync::mpsc::SyncSender};

#[derive(Debug)]
pub struct AvfCameraDriver {
    _config: CameraConfig,
    _frame_tx: SyncSender<FrameMsg>,
    _events_tx: SyncSender<CameraEvent>,
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
        Ok(Self {
            _config: config,
            _frame_tx: frame_tx,
            _events_tx: events_tx,
        })
    }
}

impl CameraDriver for AvfCameraDriver {
    fn backend(&self) -> CameraBackend {
        CameraBackend::Avf
    }

    fn start(&mut self) -> Result<(), CameraError> {
        Err(CameraError::unsupported(
            "avfoundation backend not implemented",
        ))
    }

    fn stop(&mut self) -> Result<(), CameraError> {
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
