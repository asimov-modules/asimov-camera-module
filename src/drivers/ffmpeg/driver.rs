// This is free and unencumbered software released into the public domain.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use crossbeam_channel as ch;

use crate::drivers::{CameraDriver, DriverConfig};
use crate::{CameraBackend, CameraError, RawFormat, RawFrame, RawFrameRef, RawPlane};

pub struct FfmpegDriver {
    cfg: DriverConfig,

    raw_tx: ch::Sender<RawFrameRef>,
    raw_rx: ch::Receiver<RawFrameRef>,

    child: Option<Arc<Mutex<Child>>>,
    stop: Arc<AtomicBool>,
    running: AtomicBool,
    closed: AtomicBool,

    reader_join: Option<JoinHandle<()>>,
    monitor_join: Option<JoinHandle<()>>,
}

pub fn try_open(cfg: &DriverConfig) -> Result<Box<dyn CameraDriver>, CameraError> {
    Ok(Box::new(FfmpegDriver::open(cfg)?))
}

impl core::fmt::Debug for FfmpegDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FfmpegDriver")
            .field("cfg", &self.cfg)
            .field("child", &self.child.as_ref().map(|_| "<child>"))
            .field("running", &self.running.load(Ordering::Relaxed))
            .field("closed", &self.closed.load(Ordering::Relaxed))
            .finish()
    }
}

impl FfmpegDriver {
    pub fn open(cfg: &DriverConfig) -> Result<Self, CameraError> {
        cfg.validate()?;

        let cap = cfg.buffer_raw.max(1).min(8);
        let (raw_tx, raw_rx) = ch::bounded::<RawFrameRef>(cap);

        Ok(Self {
            cfg: cfg.clone(),
            raw_tx,
            raw_rx,
            child: None,
            stop: Arc::new(AtomicBool::new(false)),
            running: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            reader_join: None,
            monitor_join: None,
        })
    }

    #[inline]
    fn now_ns_best_effort() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    fn spawn_ffmpeg(&self) -> Result<Child, CameraError> {
        let device_id = self.cfg.device.id().trim();
        if device_id.is_empty() {
            return Err(CameraError::invalid_config("ffmpeg device id is empty"));
        }

        let input_device = get_input_device(device_id);

        #[cfg(target_os = "macos")]
        let input_fps: f64 = 30.0;

        #[cfg(not(target_os = "macos"))]
        let input_fps: f64 = {
            let fps = if self.cfg.fps.is_finite() && self.cfg.fps > 0.1 {
                self.cfg.fps
            } else {
                30.0
            };
            fps.min(240.0)
        };

        let mut ffargs: Vec<String> = vec![
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
        ];

        #[cfg(target_os = "macos")]
        {
            ffargs.push("-pixel_format".into());
            ffargs.push("0rgb".into());
        }

        ffargs.extend([
            "-i".into(),
            input_device,
            "-pix_fmt".into(),
            "rgb24".into(),
            "-f".into(),
            "rawvideo".into(),
            "pipe:1".into(),
        ]);

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
        terminate_child(&mut *g);
    }

    fn join_threads(&mut self) {
        if let Some(j) = self.reader_join.take() {
            let _ = j.join();
        }
        if let Some(j) = self.monitor_join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for FfmpegDriver {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

impl CameraDriver for FfmpegDriver {
    fn backend(&self) -> CameraBackend {
        CameraBackend::Ffmpeg
    }

    fn start(&mut self) -> Result<(), CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CameraError::Closed);
        }
        if self.running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        if self.child.is_some() {
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
        let raw_tx = self.raw_tx.clone();

        let raw_rx_drop = self.raw_rx.clone();

        let reader_join = std::thread::Builder::new()
            .name("asimov-ffmpeg-reader".to_string())
            .spawn(move || {
                let mut reader = std::io::BufReader::new(stdout);
                let mut buf = vec![0u8; frame_size];

                while !stop.load(Ordering::Acquire) {
                    match reader.read_exact(&mut buf) {
                        Ok(()) => {
                            let ts = FfmpegDriver::now_ns_best_effort();

                            let plane = RawPlane::new(Bytes::copy_from_slice(&buf), row_stride, 3);

                            let frame_ref: RawFrameRef = Arc::new(RawFrame {
                                width,
                                height,
                                format: RawFormat::PackedRgb8,
                                planes: vec![plane],
                                timestamp_ns: Some(ts),
                            });

                            if raw_tx.try_send(Arc::clone(&frame_ref)).is_err() {
                                let _ = raw_rx_drop.try_recv();
                                let _ = raw_tx.try_send(frame_ref);
                            }
                        },
                        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                            break;
                        },
                        Err(_) => {
                            break;
                        },
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

        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        self.stop.store(true, Ordering::Release);

        self.stop_child();

        self.join_threads();

        self.running.store(false, Ordering::Release);
        Ok(())
    }

    fn close(&mut self) -> Result<(), CameraError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let _ = self.stop();
        Ok(())
    }

    fn read_frames(&mut self) -> Result<ch::Receiver<RawFrameRef>, CameraError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(CameraError::Closed);
        }
        Ok(self.raw_rx.clone())
    }
}

fn terminate_child(child: &mut Child) {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(200) {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(target_os = "macos")]
fn ffmpeg_format() -> &'static str {
    "avfoundation"
}

#[cfg(target_os = "linux")]
fn ffmpeg_format() -> &'static str {
    "v4l2"
}

#[cfg(target_os = "windows")]
fn ffmpeg_format() -> &'static str {
    "dshow"
}

#[cfg(target_os = "macos")]
fn get_input_device(device: &str) -> String {
    device.strip_prefix("avf:").unwrap_or(device).to_string()
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
