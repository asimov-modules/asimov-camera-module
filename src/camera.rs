// This is free and unencumbered software released into the public domain.

use crate::{
    CameraBackend, CameraConfig, CameraError, Frame, FrameRef, PixelFormat, RawFormat, RawFrameRef,
    default_device, drivers,
};

use crate::converter;
use crate::runtime::sampler::FpsSampler;

use crossbeam_channel as ch;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Camera {
    driver: Box<dyn drivers::CameraDriver>,
    frame_rx: ch::Receiver<FrameRef>,
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

    pub fn read_frames(&mut self) -> Result<ch::Receiver<FrameRef>, CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CameraError::Closed);
        }
        Ok(self.frame_rx.clone())
    }

    pub fn driver_preview_info(&self) -> Option<(u32, u32, i32)> {
        self.driver.preview_info()
    }

    fn raw_to_frame(raw: RawFrameRef) -> Option<FrameRef> {
        let r = raw.as_ref();
        if r.planes.is_empty() {
            return None;
        }

        match r.format {
            RawFormat::PackedRgb8 => {
                let p0 = &r.planes[0];

                if p0.pixel_stride != 3 {
                    return None;
                }

                Some(Arc::new(Frame::new(
                    r.width,
                    r.height,
                    p0.row_stride.max(r.width.saturating_mul(3)),
                    PixelFormat::Rgb8,
                    p0.data.clone(),
                    r.timestamp_ns,
                )))
            },

            _ => converter::convert_raw_to_frame(raw, PixelFormat::Rgb8),
        }
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

    let cap = cfg.buffer_frames.max(1);
    let (frame_tx, frame_rx) = ch::bounded::<FrameRef>(cap);
    let frame_rx_thread = frame_rx.clone();

    let (stop_tx, stop_rx) = ch::bounded::<()>(1);

    let throttle_fps = cfg.throttle_fps;
    let diagnostics = cfg.diagnostics;
    let extra_frame_tx = cfg.frame_tx.clone();

    let closed = Arc::new(AtomicBool::new(false));
    let closed_thread = Arc::clone(&closed);

    let worker = std::thread::Builder::new()
        .name("asimov-camera-dispatch".to_string())
        .spawn(move || {
            let mut sampler = throttle_fps.map(FpsSampler::new);

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

                        if let Some(s) = sampler.as_mut() {
                            if !s.should_emit() {
                                continue;
                            }
                        }

                        let Some(frame) = Camera::raw_to_frame(raw) else {
                            continue;
                        };

                        if frame_tx.try_send(frame.clone()).is_err() {
                            let _ = frame_rx_thread.try_recv();
                            let _ = frame_tx.try_send(frame.clone());
                        }

                        if let Some(tx) = extra_frame_tx.as_ref() {
                            let _ = tx.try_send(frame);
                        }

                        let _ = diagnostics;
                    }
                }
            }
        })
        .map_err(|e| CameraError::other(format!("failed to spawn camera worker: {e}")))?;

    Ok(Camera {
        driver,
        frame_rx,
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
