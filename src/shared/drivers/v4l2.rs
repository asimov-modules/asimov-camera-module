// This is free and unencumbered software released into the public domain.

use crate::shared::{
    CameraBackend, CameraConfig, CameraDriver, CameraError, CameraEvent, FrameMsg,
};
use std::{any::Any, sync::mpsc::SyncSender};

#[derive(Debug)]
pub struct V4l2CameraDriver {
    _config: CameraConfig,
    _frame_tx: SyncSender<FrameMsg>,
    _events_tx: SyncSender<CameraEvent>,
}

impl V4l2CameraDriver {
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

impl CameraDriver for V4l2CameraDriver {
    fn backend(&self) -> CameraBackend {
        CameraBackend::V4l2
    }

    fn start(&mut self) -> Result<(), CameraError> {
        Err(CameraError::unsupported("v4l2 backend not implemented"))
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
