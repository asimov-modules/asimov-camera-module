// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraConfig, CameraDriver, CameraError, Frame, FrameCallback};
use alloc::borrow::Cow;
use std::{
    io::Read,
    process::{Child, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct FfmpegCameraDriver {
    pub config: CameraConfig,
    pub process: Option<Child>,
    callback: Option<FrameCallback>,
}

impl core::fmt::Debug for FfmpegCameraDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FfmpegCameraDriver")
            .field("config", &self.config)
            .field("process", &self.process.as_ref().map(|_| "<child>"))
            .field("callback", &self.callback.as_ref().map(|_| "<callback>"))
            .finish()
    }
}

impl dogma::Named for FfmpegCameraDriver {
    fn name(&self) -> Cow<'_, str> {
        "ffmpeg".into()
    }
}

impl FfmpegCameraDriver {
    pub fn open(
        _input_url: impl AsRef<str>,
        config: CameraConfig,
        callback: FrameCallback,
    ) -> Result<Self, CameraError> {
        Ok(Self {
            config,
            process: None,
            callback: Some(callback),
        })
    }

    #[inline]
    fn now_ns_best_effort() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
}

impl CameraDriver for FfmpegCameraDriver {
    fn start(&mut self) -> Result<(), CameraError> {
        let mut cb = self.callback.take().ok_or(CameraError::NotConfigured)?;

        let mut child = spawn_reader(&self.config)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CameraError::other("ffmpeg stdout not piped"))?;

        let width = self.config.width;
        let height = self.config.height;
        let stride = width.saturating_mul(3);
        let frame_size = (stride as usize).saturating_mul(height as usize);

        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            let mut buf = vec![0u8; frame_size];

            loop {
                match reader.read_exact(&mut buf) {
                    Ok(()) => {
                        let ts = Self::now_ns_best_effort();
                        let frame = Frame::new_rgb8(buf.clone(), width, height, stride)
                            .with_timestamp_ns(ts);
                        cb(frame);
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(_) => break,
                }
            }
        });

        self.process = Some(child);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), CameraError> {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }
}

/// Spawn FFmpeg configured to read raw RGB frames from the camera and write them to stdout.
fn spawn_reader(config: &CameraConfig) -> Result<Child, CameraError> {
    let input_device = get_input_device(&config.device);

    // Intentionally hard-coded for stability for now.
    // TODO: safely honor config.fps across platforms/backends.
    const INPUT_FRAMERATE: u32 = 30;

    let mut ffargs: Vec<String> = vec![
        "-hide_banner".into(),
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
        input_device.clone(),
        "-pix_fmt".into(),
        "rgb24".into(),
        "-f".into(),
        "rawvideo".into(),
        "pipe:1".into(),
    ]);

    #[cfg(feature = "tracing")]
    asimov_module::tracing::debug!(
        target: "asimov_camera_module::driver::ffmpeg",
        device = %input_device,
        width = config.width,
        height = config.height,
        fps = config.fps,
        "spawning ffmpeg"
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.args(&ffargs)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    cmd.spawn()
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
    device
        .strip_prefix("file:/dev/video")
        .unwrap_or(device)
        .to_string()
}

#[cfg(target_os = "linux")]
fn get_input_device(device: &str) -> String {
    if device.chars().all(|c| c.is_ascii_digit()) {
        format!("/dev/video{device}")
    } else {
        device.strip_prefix("file:").unwrap_or(device).to_string()
    }
}

#[cfg(target_os = "windows")]
fn get_input_device(device: &str) -> String {
    device.to_string()
}
