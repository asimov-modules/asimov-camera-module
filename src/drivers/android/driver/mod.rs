// This is free and unencumbered software released into the public domain.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel as ch;

use crate::drivers::CameraDriver;
use crate::{CameraBackend, CameraError, RawFrameRef};

use crate::drivers::DriverConfig;

use super::ndk::{CameraDevice, CameraManager, NativeWindow};

mod callbacks;
mod image_stream;
mod open;
mod session_graph;
mod sizes;

use callbacks::State;
use image_stream::ImageStream;
use session_graph::SessionGraph;

pub struct AndroidDriver {
    _cfg: DriverConfig,

    _raw_tx: ch::Sender<RawFrameRef>,
    raw_rx: ch::Receiver<RawFrameRef>,

    _mgr: CameraManager,
    _dev: CameraDevice,

    preview_window: NativeWindow,
    image_stream: ImageStream,
    graph: SessionGraph,
    running: AtomicBool,
    closed: AtomicBool,
    _state: Arc<State>,

    picked_w: u32,
    picked_h: u32,
    rotation_deg: i32,
}

pub fn try_open(cfg: &DriverConfig) -> Result<Box<dyn CameraDriver>, CameraError> {
    open::open(cfg).map(|d| Box::new(d) as Box<dyn CameraDriver>)
}

impl AndroidDriver {
    fn teardown(&mut self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }

        let _ = self.graph.stop_repeating();
        self.graph.close();
        self.image_stream.close();
        self.preview_window = NativeWindow::default();
    }
}

impl Drop for AndroidDriver {
    fn drop(&mut self) {
        self.teardown();
    }
}

impl CameraDriver for AndroidDriver {
    fn backend(&self) -> CameraBackend {
        CameraBackend::Android
    }

    fn start(&mut self) -> Result<(), CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CameraError::Closed);
        }
        if self.running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        self.graph.start_repeating().map_err(|e| {
            self.running.store(false, Ordering::Release);
            e
        })?;

        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        if !self.running.swap(false, Ordering::AcqRel) {
            return Ok(());
        }

        self.graph.stop_repeating()?;
        Ok(())
    }

    fn close(&mut self) -> Result<(), CameraError> {
        let _ = self.stop();
        self.teardown();
        Ok(())
    }

    fn read_frames(&mut self) -> Result<ch::Receiver<RawFrameRef>, CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CameraError::Closed);
        }
        Ok(self.raw_rx.clone())
    }

    fn preview_info(&self) -> Option<(u32, u32, i32)> {
        if self.picked_w == 0 || self.picked_h == 0 {
            return None;
        }
        Some((self.picked_w, self.picked_h, self.rotation_deg))
    }
}
