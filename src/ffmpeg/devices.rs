// This is free and unencumbered software released into the public domain.

use crate::ffmpeg::{CameraError, Result};

/// Mirrors `nativecam::DeviceInfo`'s shape.
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    id: String,
    name: String,
    kind: DeviceKind,
}

impl DeviceInfo {
    fn new(id: String, name: String, kind: DeviceKind) -> Self {
        Self { id, name, kind }
    }

    #[inline]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn kind(&self) -> DeviceKind {
        self.kind
    }
}

/// Mirrors `nativecam::DeviceKind`'s shape. `Front`/`Back` never occur here
/// (no phone camera concept on Linux/Windows) but exist so shared CLI code
/// matching on them compiles regardless of which backend is in scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    External,
    #[allow(dead_code)]
    Front,
    #[allow(dead_code)]
    Back,
    Unknown,
}

pub fn list_video_devices() -> Result<Vec<DeviceInfo>> {
    #[cfg(target_os = "windows")]
    {
        return ffmpeg_list_devices_windows_dshow();
    }
    #[cfg(target_os = "linux")]
    {
        return ffmpeg_list_devices_linux_v4l2();
    }

    #[allow(unreachable_code)]
    Ok(Vec::new())
}

pub fn pick_preferred_device(devices: &[DeviceInfo]) -> Option<DeviceInfo> {
    if devices.is_empty() {
        return None;
    }
    if let Some(d) = devices.iter().find(|d| d.kind() == DeviceKind::External) {
        return Some(d.clone());
    }
    Some(devices[0].clone())
}

pub fn default_device() -> Result<Option<DeviceInfo>> {
    let devices = list_video_devices()?;
    Ok(pick_preferred_device(&devices))
}

fn ffmpeg_bin() -> String {
    std::env::var("ASIMOV_CAMERA_FFMPEG_BIN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("FFMPEG_BIN")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| "ffmpeg".to_string())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn run_ffmpeg(args: &[&str]) -> Result<(i32, String, String)> {
    use std::process::Command;

    let bin = ffmpeg_bin();
    let out = Command::new(&bin).args(args).output().map_err(|e| {
        CameraError::driver(
            "ffmpeg",
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("failed to execute '{bin}': {e}"),
            ),
        )
    })?;

    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    Ok((code, stdout, stderr))
}

#[cfg(target_os = "windows")]
fn ffmpeg_list_devices_windows_dshow() -> Result<Vec<DeviceInfo>> {
    let (_code, _stdout, stderr) = run_ffmpeg(&[
        "-hide_banner",
        "-f",
        "dshow",
        "-list_devices",
        "true",
        "-i",
        "dummy",
    ])?;

    let mut in_video = false;
    let mut out = Vec::new();

    for line in stderr.lines() {
        let s = line.trim();

        if s.contains("DirectShow video devices") {
            in_video = true;
            continue;
        }
        if s.contains("DirectShow audio devices") {
            in_video = false;
            continue;
        }
        if !in_video {
            continue;
        }

        if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
            let name = &s[1..s.len() - 1];
            if name.is_empty() {
                continue;
            }

            let n = name.to_lowercase();
            let is_ext = n.contains("usb")
                || n.contains("webcam")
                || n.contains("uvc")
                || n.contains("capture");

            out.push(DeviceInfo::new(
                name.to_string(),
                name.to_string(),
                if is_ext {
                    DeviceKind::External
                } else {
                    DeviceKind::Unknown
                },
            ));
        }
    }

    if out.is_empty() {
        return Err(CameraError::other(
            "no video devices were returned by ffmpeg (dshow)",
        ));
    }

    Ok(out)
}

#[cfg(target_os = "linux")]
fn ffmpeg_list_devices_linux_v4l2() -> Result<Vec<DeviceInfo>> {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn sysfs_video_dir(idx: u32) -> PathBuf {
        PathBuf::from("/sys/class/video4linux").join(format!("video{idx}"))
    }

    fn sysfs_name(sys: &Path, fallback: &str) -> String {
        fs::read_to_string(sys.join("name"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }

    fn sysfs_is_usb(sys: &Path) -> bool {
        let dev = sys.join("device");
        if let Ok(link) = fs::read_link(&dev) {
            if link.to_string_lossy().to_lowercase().contains("usb") {
                return true;
            }
        }
        if let Ok(uevent) = fs::read_to_string(dev.join("uevent")) {
            if uevent.to_lowercase().contains("usb") {
                return true;
            }
        }
        false
    }

    let mut idxs: Vec<u32> = Vec::new();
    if let Ok(entries) = fs::read_dir("/dev") {
        for e in entries.flatten() {
            let name = e.file_name();
            let s = name.to_string_lossy();
            if s.starts_with("video") && s[5..].chars().all(|c| c.is_ascii_digit()) {
                if let Ok(v) = s[5..].parse::<u32>() {
                    idxs.push(v);
                }
            }
        }
    }
    idxs.sort_unstable();
    idxs.dedup();

    let mut out = Vec::new();

    for idx in idxs {
        let devnode = format!("/dev/video{idx}");
        if !Path::new(&devnode).exists() {
            continue;
        }

        let (_code, _stdout, stderr) = run_ffmpeg(&[
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "v4l2",
            "-i",
            &devnode,
            "-t",
            "0.1",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])?;

        if stderr.contains("No such file") || stderr.contains("No such device") {
            continue;
        }

        let sys = sysfs_video_dir(idx);
        let name = sysfs_name(&sys, &devnode);
        let is_usb = sysfs_is_usb(&sys);

        out.push(DeviceInfo::new(
            devnode.clone(),
            name,
            if is_usb {
                DeviceKind::External
            } else {
                DeviceKind::Unknown
            },
        ));
    }

    if out.is_empty() {
        return Err(CameraError::other(
            "no usable v4l2 video devices were found via ffmpeg",
        ));
    }

    Ok(out)
}
