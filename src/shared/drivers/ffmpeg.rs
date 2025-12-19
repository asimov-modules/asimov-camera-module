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
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct FfmpegCameraDriver {
    config: CameraConfig,
    child: Option<Child>,
    stop: Arc<AtomicBool>,
    reader_join: Option<JoinHandle<()>>,
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
        if let Some(mut child) = self.child.take() {
            #[cfg(unix)]
            {
                unsafe {
                    let _ = libc::kill(child.id() as i32, libc::SIGTERM);
                }
                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_millis(900) {
                    if let Ok(Some(_)) = child.try_wait() {
                        break;
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

        let stop = Arc::clone(&self.stop);
        let frame_tx = self.frame_tx.clone();
        let events_tx = self.events_tx.clone();

        let join = std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);

            while !stop.load(Ordering::Relaxed) {
                let mut buf = vec![0u8; frame_size];
                match reader.read_exact(&mut buf) {
                    Ok(()) => {
                        let ts = FfmpegCameraDriver::now_ns_best_effort();
                        let frame = Frame::new_rgb8(Bytes::from(buf), width, height, stride)
                            .with_timestamp_ns(ts);
                        try_send_frame(&frame_tx, &events_tx, CameraBackend::Ffmpeg, frame);
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
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

        self.reader_join = Some(join);
        self.child = Some(child);

        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        self.stop.store(true, Ordering::Relaxed);
        self.stop_child();
        if let Some(j) = self.reader_join.take() {
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

    const INPUT_FRAMERATE: u32 = 30;

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
        INPUT_FRAMERATE.to_string(),
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
