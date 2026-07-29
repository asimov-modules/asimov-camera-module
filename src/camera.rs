// This is free and unencumbered software released into the public domain.

use crate::{
    CameraBackend, CameraConfig, CameraError, CameraState, FrameRef, PixelFormat, RawFrameRef,
    SubscribeOptions, default_device, drivers,
};

use crate::dispatch::FrameDistributor;

use crossbeam_channel as ch;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub struct Camera {
    driver: Box<dyn drivers::CameraDriver>,
    distributor: Arc<FrameDistributor>,
    stop_tx: ch::Sender<()>,
    worker: Option<std::thread::JoinHandle<()>>,

    /// Authoritative lifecycle state (see `CameraState`). Only touched by
    /// `start`/`stop`/`close`/`subscribe*`, all cold-path calls, so a mutex
    /// here is free.
    state: Mutex<CameraState>,

    /// Cheap hot-path mirror of `state == Closed`, polled once per dispatched
    /// frame by the worker thread. An atomic load there costs nothing; taking
    /// `state`'s mutex on every frame would not be free.
    closed: Arc<AtomicBool>,
}

impl Camera {
    pub fn backend(&self) -> CameraBackend {
        self.driver.backend()
    }

    /// Current lifecycle state.
    pub fn state(&self) -> CameraState {
        *self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Begin capturing, or resume after `stop()`. A no-op if already
    /// `Running`. Fails with `CameraError::Closed` if the camera is closed.
    pub fn start(&mut self) -> Result<(), CameraError> {
        {
            let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
            match *st {
                CameraState::Closed => return Err(CameraError::Closed),
                CameraState::Running => return Ok(()),
                CameraState::Idle | CameraState::Stopped => *st = CameraState::Running,
            }
        }
        self.driver.start()
    }

    /// Pause capturing without releasing the underlying device/session, so a
    /// later `start()` resumes cheaply. A no-op unless currently `Running`.
    pub fn stop(&mut self) -> Result<(), CameraError> {
        {
            let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
            match *st {
                CameraState::Running => *st = CameraState::Stopped,
                _ => return Ok(()),
            }
        }
        self.driver.stop()
    }

    /// Release the camera. Terminal — no further calls are valid afterwards.
    /// Idempotent.
    pub fn close(&mut self) -> Result<(), CameraError> {
        {
            let mut st = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if *st == CameraState::Closed {
                return Ok(());
            }
            *st = CameraState::Closed;
        }
        self.closed.store(true, Ordering::Release);

        let _ = self.stop_tx.try_send(());
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }

        let _ = self.driver.stop();
        self.driver.close()
    }

    /// Subscribe to frames converted to `format`. Legal in any state except
    /// `Closed` — subscribing before `start()` is fine, and lets you avoid
    /// missing the first frames once capture begins. Can be called any
    /// number of times, with the same or different formats; each call gets
    /// its own independent channel.
    pub fn subscribe(
        &self,
        format: PixelFormat,
        opts: SubscribeOptions,
    ) -> Result<ch::Receiver<FrameRef>, CameraError> {
        if self.state() == CameraState::Closed {
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
        if self.state() == CameraState::Closed {
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
        state: Mutex::new(CameraState::Idle),
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
