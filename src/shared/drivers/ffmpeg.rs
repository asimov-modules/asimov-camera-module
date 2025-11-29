// This is free and unencumbered software released into the public domain.

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
compile_error!("FFmpeg camera driver currently supports only macOS, Linux and Windows.");

use crate::core::{Error, Result};
use crate::shared::{CameraConfig, CameraDriver};
use std::process::{Child, Command, Stdio};

#[derive(Clone, Debug)]
pub struct FfmpegCameraDriver {}

impl CameraDriver for FfmpegCameraDriver {}

/// Spawn FFmpeg configured to read raw RGB frames from the camera and write them to stdout.
///
/// The child process is configured to:
///   - use platform-specific input format (`ffmpeg_format()`)
///   - use the correct input device mapping (`get_input_device`)
///   - output `rgb24` rawvideo frames to `pipe:1`
pub fn spawn_reader(config: &CameraConfig) -> Result<Child> {
    let input_device = get_input_device(&config.device);
    let fps_s = config.fps.to_string();

    let mut ffargs: Vec<String> = vec![
        "-hide_banner".into(),
        "-f".into(),
        ffmpeg_format().into(),
        "-loglevel".into(),
        "error".into(),
        "-video_size".into(),
        format!("{}x{}", config.width, config.height),
        "-framerate".into(),
        fps_s.clone(),
    ];

    #[cfg(target_os = "macos")]
    {
        ffargs.push("-pixel_format".into());
        ffargs.push("0rgb".into());
    }

    ffargs.extend([
        "-i".into(),
        input_device.clone(),
        "-preset".into(),
        "veryfast".into(),
        "-tune".into(),
        "zerolatency".into(),
        "-vf".into(),
        format!("fps={}", fps_s),
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
        .map_err(|e| Error::FfmpegSpawn(format!("failed to spawn ffmpeg: {e}")))
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
