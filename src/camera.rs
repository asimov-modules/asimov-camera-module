// This is free and unencumbered software released into the public domain.

use crate::{
    CameraBackend, CameraConfig, CameraError, FrameRef, PixelFormat, RawFrameRef, SubscribeOptions,
    default_device, drivers,
};

use crate::dispatch::FrameDistributor;

use crossbeam_channel as ch;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Camera {
    driver: Box<dyn drivers::CameraDriver>,
    distributor: Arc<FrameDistributor>,
    stop_tx: ch::Sender<()>,
    worker: Option<std::thread::JoinHandle<()>>,
    closed: Arc<AtomicBool>,
}

impl Camera {
    pub fn backend(&self) -> CameraBackend {
        self.driver.backend()
    }

    pub fn start(&mut self) -> Result<(), CameraError> {
        self.driver.start()
    }

    pub fn stop(&mut self) -> Result<(), CameraError> {
        self.driver.stop()
    }

    pub fn close(&mut self) -> Result<(), CameraError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        let _ = self.stop_tx.try_send(());
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }

        let _ = self.stop();
        self.driver.close()
    }

    /// Subscribe to frames converted to `format`. Can be called any number of
    /// times, before or after `start()`, with the same or different formats;
    /// each call gets its own independent channel.
    pub fn subscribe(
        &self,
        format: PixelFormat,
        opts: SubscribeOptions,
    ) -> Result<ch::Receiver<FrameRef>, CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CameraError::Closed);
        }
        Ok(self.distributor.subscribe_converted(format, opts))
    }

    /// Subscribe to raw, unconverted frames exactly as the driver produced
    /// them (see `RawFormat` for what that means per platform).
    pub fn subscribe_raw(
        &self,
        opts: SubscribeOptions,
    ) -> Result<ch::Receiver<RawFrameRef>, CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CameraError::Closed);
        }
        Ok(self.distributor.subscribe_raw(opts))
    }

    pub fn driver_preview_info(&self) -> Option<(u32, u32, i32)> {
        self.driver.preview_info()
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

pub fn open_camera(mut cfg: CameraConfig) -> Result<Camera, CameraError> {
    cfg = cfg.normalized();

    if cfg.device.is_none() {
        cfg.device = default_device()?;
    }
    if cfg.device.is_none() {
        return Err(CameraError::NoCamera);
    }

    cfg.validate()?;

    let driver_cfg = build_driver_config(&cfg)?;
    let mut driver = drivers::open(&driver_cfg)?;

    let raw_rx = driver.read_frames()?;

    let (stop_tx, stop_rx) = ch::bounded::<()>(1);

    let distributor = Arc::new(FrameDistributor::new());
    let distributor_thread = Arc::clone(&distributor);

    let closed = Arc::new(AtomicBool::new(false));
    let closed_thread = Arc::clone(&closed);

    let worker = std::thread::Builder::new()
        .name("asimov-camera-dispatch".to_string())
        .spawn(move || {
            loop {
                ch::select! {
                    recv(stop_rx) -> _ => break,

                    recv(raw_rx) -> msg => {
                        let mut raw = match msg {
                            Ok(v) => v,
                            Err(_) => break,
                        };

                        while let Ok(next) = raw_rx.try_recv() {
                            raw = next;
                        }

                        if closed_thread.load(Ordering::Relaxed) {
                            continue;
                        }

                        distributor_thread.dispatch(raw);
                    }
                }
            }
        })
        .map_err(|e| CameraError::other(format!("failed to spawn camera worker: {e}")))?;

    Ok(Camera {
        driver,
        distributor,
        stop_tx,
        worker: Some(worker),
        closed,
    })
}

fn build_driver_config(cfg: &CameraConfig) -> Result<drivers::DriverConfig, CameraError> {
    let device = cfg
        .device
        .clone()
        .ok_or_else(|| CameraError::invalid_config("device must be resolved before driver open"))?;

    #[cfg(all(feature = "mobile-preview", feature = "android", target_os = "android"))]
    {
        let dc = drivers::DriverConfig {
            device,
            width: cfg.width,
            height: cfg.height,
            fps: cfg.fps,
            buffer_raw: cfg.buffer_raw,
            diagnostics: cfg.diagnostics,
            android_preview: cfg.android_preview,
        }
        .normalized();

        dc.validate()?;
        return Ok(dc);
    }

    #[cfg(not(all(feature = "mobile-preview", feature = "android", target_os = "android")))]
    {
        let dc = drivers::DriverConfig {
            device,
            width: cfg.width,
            height: cfg.height,
            fps: cfg.fps,
            buffer_raw: cfg.buffer_raw,
            diagnostics: cfg.diagnostics,
        }
        .normalized();

        dc.validate()?;
        Ok(dc)
    }
}

#[cfg(all(feature = "mobile-preview", feature = "avf", target_os = "ios"))]
impl Camera {
    pub fn session_handle(&self) -> Result<crate::AvfSessionHandle, CameraError> {
        self.driver.session_handle()
    }
}
