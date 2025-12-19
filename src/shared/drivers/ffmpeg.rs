// This is free and unencumbered software released into the public domain.

use crate::shared::{
    CameraBackend, CameraConfig, CameraDriver, CameraError, CameraEvent, Frame, FrameMsg,
    try_send_frame,
};
use bytes::Bytes;
use std::{
    any::Any,
    env,
    io::Read,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct FfmpegCameraDriver {
    config: CameraConfig,
    child: Option<Arc<Mutex<Child>>>,
    stop: Arc<AtomicBool>,
    reader_join: Option<JoinHandle<()>>,
    monitor_join: Option<JoinHandle<()>>,
    frame_tx: SyncSender<FrameMsg>,
    events_tx: SyncSender<CameraEvent>,
}

impl core::fmt::Debug for FfmpegCameraDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FfmpegCameraDriver")
            .field("config", &self.config)
            .field("child", &self.child.as_ref().map(|_| "<child>"))
            .finish()
    }
}

impl FfmpegCameraDriver {
    pub fn open(
        _input_url: impl AsRef<str>,
        config: CameraConfig,
        frame_tx: SyncSender<FrameMsg>,
        events_tx: SyncSender<CameraEvent>,
    ) -> Result<Self, CameraError> {
        Ok(Self {
            config,
            child: None,
            stop: Arc::new(AtomicBool::new(false)),
            reader_join: None,
            monitor_join: None,
            frame_tx,
            events_tx,
        })
    }

    #[inline]
    fn now_ns_best_effort() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    fn spawn(&self) -> Result<Child, CameraError> {
        spawn_reader(&self.config)
    }

    fn stop_child(&mut self) {
        let Some(child_arc) = self.child.take() else {
            return;
        };
        if let Ok(mut g) = child_arc.lock() {
            terminate_child(&mut *g);
        }
    }
}

impl CameraDriver for FfmpegCameraDriver {
    fn backend(&self) -> CameraBackend {
        CameraBackend::Ffmpeg
    }

    fn start(&mut self) -> Result<(), CameraError> {
        if self.child.is_some() {
            return Ok(());
        }

        self.stop.store(false, Ordering::Relaxed);

        let mut child = self.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CameraError::other("ffmpeg stdout not piped"))?;

        let width = self.config.width;
        let height = self.config.height;
        let stride = width.saturating_mul(3);
        let frame_size = (stride as usize).saturating_mul(height as usize);

        let child_arc = Arc::new(Mutex::new(child));
        self.child = Some(Arc::clone(&child_arc));

        let stop = Arc::clone(&self.stop);
        let frame_tx = self.frame_tx.clone();
        let events_tx = self.events_tx.clone();

        let reader_join = std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            let mut buf = vec![0u8; frame_size];

            while !stop.load(Ordering::Relaxed) {
                match reader.read_exact(&mut buf) {
                    Ok(()) => {
                        let ts = FfmpegCameraDriver::now_ns_best_effort();
                        let frame =
                            Frame::new_rgb8(Bytes::copy_from_slice(&buf), width, height, stride)
                                .with_timestamp_ns(ts);
                        try_send_frame(&frame_tx, &events_tx, CameraBackend::Ffmpeg, frame);
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        let _ = events_tx.try_send(CameraEvent::Error {
                            backend: CameraBackend::Ffmpeg,
                            error: CameraError::other("ffmpeg stream ended (EOF)"),
                        });
                        break;
                    },
                    Err(e) => {
                        let _ = events_tx.try_send(CameraEvent::Error {
                            backend: CameraBackend::Ffmpeg,
                            error: CameraError::driver("ffmpeg read", e),
                        });
                        break;
                    },
                }
            }
        });

        let stop2 = Arc::clone(&self.stop);
        let events_tx2 = self.events_tx.clone();
        let child_arc2 = Arc::clone(&child_arc);

        let monitor_join = std::thread::spawn(move || {
            while !stop2.load(Ordering::Relaxed) {
                let status = {
                    let mut g = match child_arc2.lock() {
                        Ok(v) => v,
                        Err(p) => p.into_inner(),
                    };
                    g.try_wait()
                };

                match status {
                    Ok(Some(s)) => {
                        // If we are stopping intentionally, don't spam as "error".
                        if stop2.load(Ordering::Relaxed) {
                            break;
                        }
                        let _ = events_tx2.try_send(CameraEvent::Error {
                            backend: CameraBackend::Ffmpeg,
                            error: CameraError::other(format!("ffmpeg exited: {}", format_exit(s))),
                        });
                        break;
                    },
                    Ok(None) => std::thread::sleep(Duration::from_millis(150)),
                    Err(e) => {
                        if stop2.load(Ordering::Relaxed) {
                            break;
                        }
                        let _ = events_tx2.try_send(CameraEvent::Error {
                            backend: CameraBackend::Ffmpeg,
                            error: CameraError::driver("ffmpeg wait", e),
                        });
                        break;
                    },
                }
            }
        });

        self.reader_join = Some(reader_join);
        self.monitor_join = Some(monitor_join);

        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        self.stop.store(true, Ordering::Relaxed);
        self.stop_child();

        if let Some(j) = self.reader_join.take() {
            let _ = j.join();
        }
        if let Some(j) = self.monitor_join.take() {
            let _ = j.join();
        }

        let _ = self.frame_tx.try_send(FrameMsg::Stop);
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Drop for FfmpegCameraDriver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn spawn_reader(config: &CameraConfig) -> Result<Child, CameraError> {
    let device = config.device.as_deref().unwrap_or("").trim();
    let input_device = get_input_device(device);

    // On macOS/AVFoundation, many devices reject "odd" framerates even when listed.
    // For a stable CLI, keep capture at a safe default and let the reader throttle output.
    #[cfg(target_os = "macos")]
    let input_fps: f64 = 30.0;

    #[cfg(not(target_os = "macos"))]
    let input_fps: f64 = {
        let fps = if config.fps.is_finite() && config.fps > 0.1 {
            config.fps
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
        format!("{}x{}", config.width, config.height),
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

    let stderr = if config.diagnostics || env::var_os("ASIMOV_CAMERA_FFMPEG_STDERR").is_some() {
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

fn format_exit(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        format!("code={code}")
    } else {
        "terminated".to_string()
    }
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        unsafe {
            let _ = libc::kill(child.id() as i32, libc::SIGTERM);
        }
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(900) {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(windows)]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
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
