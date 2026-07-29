// This is free and unencumbered software released into the public domain.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use crossbeam_channel as ch;

use crate::ffmpeg::{
    CameraConfig, CameraError, Frame, FrameRef, PixelFormat, Result, SubscribeOptions,
};

/// Mirrors `nativecam::engine::sampler::FpsSampler`'s shape.
struct FpsSampler {
    period: Duration,
    next_deadline: Instant,
}

impl FpsSampler {
    fn new(fps: f64) -> Self {
        let fps = if fps.is_finite() && fps > 0.0 {
            fps
        } else {
            30.0
        };
        let period = Duration::from_secs_f64(1.0 / fps);
        Self {
            period,
            next_deadline: Instant::now() + period,
        }
    }

    fn should_emit(&mut self) -> bool {
        let now = Instant::now();
        if now < self.next_deadline {
            return false;
        }
        self.next_deadline = if now >= self.next_deadline {
            now + self.period
        } else {
            self.next_deadline + self.period
        };
        true
    }
}

struct Sink {
    tx: ch::Sender<FrameRef>,
    rx_probe: ch::Receiver<FrameRef>,
    sampler: Option<FpsSampler>,
}

/// Mirrors `nativecam::Camera`'s shape (the subset the CLI needs): open,
/// start, stop, subscribe. Always emits `PixelFormat::Rgb8` — that's what's
/// requested from the `ffmpeg` process itself, no conversion layer needed.
pub struct Camera {
    cfg: CameraConfig,
    sinks: Arc<Mutex<Vec<Sink>>>,

    child: Option<Arc<Mutex<Child>>>,
    stop: Arc<AtomicBool>,
    running: bool,
    closed: bool,

    reader_join: Option<JoinHandle<()>>,
    monitor_join: Option<JoinHandle<()>>,
}

impl Camera {
    pub fn open(cfg: CameraConfig) -> Result<Self> {
        Ok(Self {
            cfg,
            sinks: Arc::new(Mutex::new(Vec::new())),
            child: None,
            stop: Arc::new(AtomicBool::new(false)),
            running: false,
            closed: false,
            reader_join: None,
            monitor_join: None,
        })
    }

    pub fn subscribe(
        &self,
        format: PixelFormat,
        opts: SubscribeOptions,
    ) -> Result<ch::Receiver<FrameRef>> {
        if self.closed {
            return Err(CameraError::Closed);
        }
        if format != PixelFormat::Rgb8 {
            return Err(CameraError::Other(
                "the ffmpeg backend only ever produces Rgb8".to_string(),
            ));
        }

        let (tx, rx) = ch::bounded(opts.capacity.max(1));
        let sink = Sink {
            tx,
            rx_probe: rx.clone(),
            sampler: opts.throttle_fps.map(FpsSampler::new),
        };
        self.sinks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(sink);
        Ok(rx)
    }

    #[inline]
    fn now_ns_best_effort() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    fn spawn_ffmpeg(&self) -> Result<Child> {
        let device_id = self
            .cfg
            .device
            .as_ref()
            .map(|d| d.id().trim().to_string())
            .unwrap_or_default();
        if device_id.is_empty() {
            return Err(CameraError::invalid_config("ffmpeg device id is empty"));
        }

        let input_device = get_input_device(&device_id);

        let input_fps = if self.cfg.fps.is_finite() && self.cfg.fps > 0.1 {
            self.cfg.fps
        } else {
            30.0
        }
        .min(240.0);

        let ffargs: Vec<String> = vec![
            "-hide_banner".into(),
            "-nostdin".into(),
            "-nostats".into(),
            "-f".into(),
            ffmpeg_format().into(),
            "-loglevel".into(),
            "error".into(),
            "-video_size".into(),
            format!("{}x{}", self.cfg.width, self.cfg.height),
            "-framerate".into(),
            format!("{input_fps}"),
            "-i".into(),
            input_device,
            "-pix_fmt".into(),
            "rgb24".into(),
            "-f".into(),
            "rawvideo".into(),
            "pipe:1".into(),
        ];

        let stderr =
            if self.cfg.diagnostics || std::env::var_os("ASIMOV_CAMERA_FFMPEG_STDERR").is_some() {
                Stdio::inherit()
            } else {
                Stdio::null()
            };

        Command::new("ffmpeg")
            .args(&ffargs)
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .map_err(|e| CameraError::driver("spawning ffmpeg", e))
    }

    fn stop_child(&mut self) {
        let Some(child_arc) = self.child.take() else {
            return;
        };
        let mut g = child_arc.lock().unwrap_or_else(|p| p.into_inner());
        terminate_child(&mut g);
    }

    fn join_threads(&mut self) {
        if let Some(j) = self.reader_join.take() {
            let _ = j.join();
        }
        if let Some(j) = self.monitor_join.take() {
            let _ = j.join();
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.closed {
            return Err(CameraError::Closed);
        }
        if self.running {
            return Ok(());
        }

        self.stop.store(false, Ordering::Release);

        let mut child = self.spawn_ffmpeg()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CameraError::other("ffmpeg stdout not piped"))?;

        let width = self.cfg.width;
        let height = self.cfg.height;

        let row_stride = width.saturating_mul(3);
        let frame_size = (row_stride as usize).saturating_mul(height as usize);

        let child_arc = Arc::new(Mutex::new(child));
        self.child = Some(Arc::clone(&child_arc));

        let stop = Arc::clone(&self.stop);
        let sinks = Arc::clone(&self.sinks);

        let reader_join = std::thread::Builder::new()
            .name("asimov-ffmpeg-reader".to_string())
            .spawn(move || {
                let mut reader = std::io::BufReader::new(stdout);
                let mut buf = vec![0u8; frame_size];

                while !stop.load(Ordering::Acquire) {
                    match reader.read_exact(&mut buf) {
                        Ok(()) => {
                            let ts = Self::now_ns_best_effort();

                            let frame_ref: FrameRef = Arc::new(Frame {
                                width,
                                height,
                                stride: row_stride,
                                pixel_format: PixelFormat::Rgb8,
                                data: Bytes::copy_from_slice(&buf),
                                timestamp_ns: Some(ts),
                            });

                            let mut sinks = sinks.lock().unwrap_or_else(|p| p.into_inner());
                            sinks.retain_mut(|sink| {
                                if let Some(s) = sink.sampler.as_mut() {
                                    if !s.should_emit() {
                                        return true;
                                    }
                                }
                                send_latest(&sink.tx, &sink.rx_probe, frame_ref.clone())
                            });
                        },
                        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| CameraError::other(format!("failed to spawn ffmpeg reader: {e}")))?;

        let stop2 = Arc::clone(&self.stop);
        let child_arc2 = Arc::clone(&child_arc);

        let monitor_join = std::thread::Builder::new()
            .name("asimov-ffmpeg-monitor".to_string())
            .spawn(move || {
                while !stop2.load(Ordering::Acquire) {
                    let status = {
                        let mut g = child_arc2.lock().unwrap_or_else(|p| p.into_inner());
                        g.try_wait()
                    };

                    match status {
                        Ok(Some(_)) => break,
                        Ok(None) => std::thread::sleep(Duration::from_millis(150)),
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| CameraError::other(format!("failed to spawn ffmpeg monitor: {e}")))?;

        self.reader_join = Some(reader_join);
        self.monitor_join = Some(monitor_join);
        self.running = true;

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        self.stop_child();
        self.join_threads();
        self.running = false;
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.stop()
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Send the latest frame, evicting a stale queued one on backpressure instead
/// of blocking or dropping the newest frame — mirrors
/// `nativecam::engine::dispatch`'s same pattern.
fn send_latest<T: Clone>(tx: &ch::Sender<T>, rx_probe: &ch::Receiver<T>, item: T) -> bool {
    if tx.try_send(item.clone()).is_ok() {
        return true;
    }
    let _ = rx_probe.try_recv();
    !matches!(tx.try_send(item), Err(ch::TrySendError::Disconnected(_)))
}

fn terminate_child(child: &mut Child) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(200) {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(target_os = "linux")]
fn ffmpeg_format() -> &'static str {
    "v4l2"
}

#[cfg(target_os = "windows")]
fn ffmpeg_format() -> &'static str {
    "dshow"
}

#[cfg(target_os = "linux")]
fn get_input_device(device: &str) -> String {
    let d = device.strip_prefix("file:").unwrap_or(device);
    if d.chars().all(|c| c.is_ascii_digit()) {
        format!("/dev/video{d}")
    } else {
        d.to_string()
    }
}

#[cfg(target_os = "windows")]
fn get_input_device(device: &str) -> String {
    device.strip_prefix("dshow:").unwrap_or(device).to_string()
}
