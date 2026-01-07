// This is free and unencumbered software released into the public domain.

use crate::{CameraError, drivers};

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    id: String,
    name: String,
    kind: DeviceKind,
}

impl DeviceInfo {
    pub(crate) fn new(id: String, name: String, kind: DeviceKind) -> Self {
        Self { id, name, kind }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> DeviceKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    External,
    Front,
    Back,
    Unknown,
}

/// Public: list devices for the current platform/backend selection.
pub fn list_video_devices() -> Result<Vec<DeviceInfo>, CameraError> {
    let mut devices = list_video_devices_impl()?;

    devices.retain(|d| !is_screen_capture_like(d));

    Ok(devices)
}

pub fn pick_preferred_device(devices: &[DeviceInfo]) -> Option<DeviceInfo> {
    if devices.is_empty() {
        return None;
    }

    if let Some(d) = devices
        .iter()
        .find(|d| d.kind() == DeviceKind::External && !is_continuity_like(d))
    {
        return Some(d.clone());
    }

    if let Some(d) = devices.iter().find(|d| d.kind() == DeviceKind::Front) {
        return Some(d.clone());
    }

    if let Some(d) = devices.iter().find(|d| d.kind() == DeviceKind::Back) {
        return Some(d.clone());
    }

    if let Some(d) = devices
        .iter()
        .find(|d| d.kind() == DeviceKind::External && is_continuity_like(d))
    {
        return Some(d.clone());
    }

    Some(devices[0].clone())
}

pub fn default_device() -> Result<Option<DeviceInfo>, CameraError> {
    let devices = list_video_devices()?;
    Ok(pick_preferred_device(&devices))
}

pub fn select_kind_strict(kind: DeviceKind) -> Result<String, CameraError> {
    if kind == DeviceKind::Unknown {
        return Err(CameraError::invalid_config(
            "DeviceKind::Unknown is not selectable",
        ));
    }

    let devices = list_video_devices()?;
    if let Some(d) = devices.iter().find(|d| d.kind == kind) {
        return Ok(d.id.clone());
    }

    let label = match kind {
        DeviceKind::External => "external",
        DeviceKind::Front => "front",
        DeviceKind::Back => "back",
        DeviceKind::Unknown => "unknown",
    };

    Err(CameraError::other(format!(
        "no {label} camera found; call list_video_devices() to inspect available devices"
    )))
}

pub fn require_device_id(id: &str) -> Result<String, CameraError> {
    let s = id.trim();
    if s.is_empty() {
        return Err(CameraError::invalid_config("device id is empty"));
    }
    Ok(s.to_string())
}

pub fn normalize_device_id(raw: &str) -> String {
    raw.trim().to_string()
}

fn is_screen_capture_like(d: &DeviceInfo) -> bool {
    let name = d.name().trim().to_ascii_lowercase();
    let id = d.id().trim().to_ascii_lowercase();

    name.contains("capture screen")
        || name.starts_with("capture screen")
        || name.contains("screen capture")
        || name.starts_with("screen ")
        || id.contains("capture screen")
        || id.contains("screen capture")
}

fn is_continuity_like(d: &DeviceInfo) -> bool {
    let name = d.name().trim().to_ascii_lowercase();

    name.contains("iphone")
        || name.contains("ipad")
        || name.contains("desk view")
        || name.contains("continuity")
}

#[cfg(all(feature = "android", target_os = "android"))]
fn list_video_devices_impl() -> Result<Vec<DeviceInfo>, CameraError> {
    drivers::android::devices::list_video_devices()
}

#[cfg(all(feature = "avf", any(target_os = "ios", target_os = "macos")))]
fn list_video_devices_impl() -> Result<Vec<DeviceInfo>, CameraError> {
    drivers::avf::devices::list_video_devices()
}

#[cfg(all(
    feature = "ffmpeg",
    any(target_os = "macos", target_os = "windows", target_os = "linux"),
    not(all(target_os = "macos", feature = "avf")),
))]
fn list_video_devices_impl() -> Result<Vec<DeviceInfo>, CameraError> {
    drivers::ffmpeg::devices::list_video_devices()
}

#[cfg(not(any(
    all(feature = "android", target_os = "android"),
    all(feature = "avf", any(target_os = "ios", target_os = "macos")),
    all(
        feature = "ffmpeg",
        any(target_os = "macos", target_os = "windows", target_os = "linux"),
        not(all(target_os = "macos", feature = "avf")),
    ),
)))]
fn list_video_devices_impl() -> Result<Vec<DeviceInfo>, CameraError> {
    Ok(Vec::new())
}
