// This is free and unencumbered software released into the public domain.

use crate::shared::CameraError;
use clientele::StandardOptions;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_usb: bool,
}

pub fn list_video_devices(flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    #[cfg(target_os = "macos")]
    {
        return macos_list_video_devices(flags);
    }
    #[cfg(target_os = "linux")]
    {
        return linux_list_video_devices(flags);
    }
    #[cfg(target_os = "windows")]
    {
        return windows_list_video_devices(flags);
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = flags;
        return Ok(Vec::new());
    }
}

pub fn auto_select_device(
    flags: &StandardOptions,
    preferred: Option<String>,
) -> Result<Option<String>, CameraError> {
    if let Some(p) = preferred {
        return Ok(Some(normalize_device_id(&p)));
    }

    let devices = list_video_devices(flags)?;
    if devices.is_empty() {
        return Ok(None);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(id) = macos_prefer_usb(&devices) {
            return Ok(Some(id));
        }
    }

    if let Some(d) = devices.iter().find(|d| d.is_usb) {
        return Ok(Some(d.id.clone()));
    }

    Ok(Some(devices[0].id.clone()))
}

pub fn normalize_device_id(raw: &str) -> String {
    let s = raw.trim();

    if s.starts_with("avf:") || s.starts_with("file:") || s.starts_with("dshow:") {
        return s.to_string();
    }

    #[cfg(target_os = "macos")]
    {
        if s.chars().all(|c| c.is_ascii_digit()) {
            return format!("avf:{s}");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if s.starts_with("/dev/video") {
            return format!("file:{s}");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if s.starts_with("video=") {
            return format!("dshow:{s}");
        }
        if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
            let inner = &s[1..s.len() - 1];
            if !inner.is_empty() {
                return format!("dshow:video={inner}");
            }
        }
    }

    s.to_string()
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

#[cfg(target_os = "macos")]
fn macos_list_video_devices(flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    use std::process::Command;

    if flags.debug || flags.verbose >= 2 {
        eprintln!("INFO: listing macOS AVFoundation devices via ffmpeg");
    }

    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
        .map_err(|e| CameraError::driver("running ffmpeg -list_devices", e))?;

    let stderr = String::from_utf8_lossy(&out.stderr);
    let avf = parse_avfoundation_video_devices(&stderr).unwrap_or_default();

    let usb_names = macos_usb_product_names().unwrap_or_default();

    let mut devs = Vec::new();
    for d in avf {
        let is_usb = usb_names.iter().any(|u| contains_case_insensitive(&d.name, u));
        devs.push(DeviceInfo {
            id: format!("avf:{}", d.index),
            name: d.name,
            is_usb,
        });
    }

    Ok(devs)
}

#[cfg(target_os = "macos")]
fn macos_prefer_usb(devices: &[DeviceInfo]) -> Option<String> {
    let usb_names = macos_usb_product_names().unwrap_or_default();
    if usb_names.is_empty() {
        return None;
    }
    for d in devices {
        if usb_names.iter().any(|u| contains_case_insensitive(&d.name, u)) {
            return Some(d.id.clone());
        }
    }
    None
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct AvfVideoDevice {
    index: u32,
    name: String,
}

#[cfg(target_os = "macos")]
fn parse_avfoundation_video_devices(s: &str) -> Option<Vec<AvfVideoDevice>> {
    let mut devices = Vec::new();
    let mut in_video = false;

    for line in s.lines() {
        if line.contains("AVFoundation video devices:") {
            in_video = true;
            continue;
        }
        if line.contains("AVFoundation audio devices:") {
            break;
        }
        if !in_video {
            continue;
        }

        let Some(pos) = line.find("] [") else { continue };
        let tail = line[pos + 2..].trim();

        if !tail.starts_with('[') {
            continue;
        }
        let Some(end_bracket) = tail.find(']') else { continue };

        let idx_str = &tail[1..end_bracket];
        let idx: u32 = match idx_str.trim().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let name = tail[end_bracket + 1..].trim();
        if name.is_empty() {
            continue;
        }

        devices.push(AvfVideoDevice {
            index: idx,
            name: name.to_string(),
        });
    }

    if devices.is_empty() { None } else { Some(devices) }
}

#[cfg(target_os = "macos")]
fn macos_usb_product_names() -> Option<Vec<String>> {
    let out = std::process::Command::new("ioreg")
        .args(["-p", "IOUSB", "-l"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&out.stdout);
    let mut names = Vec::new();

    for line in s.lines() {
        let line = line.trim();
        if let Some(v) = extract_quoted_value(line, "\"USB Product Name\"") {
            names.push(v);
        } else if let Some(v) = extract_quoted_value(line, "\"kUSBProductString\"") {
            names.push(v);
        }
    }

    names.sort();
    names.dedup();

    if names.is_empty() { None } else { Some(names) }
}

#[cfg(target_os = "macos")]
fn extract_quoted_value(line: &str, key: &str) -> Option<String> {
    if !line.contains(key) {
        return None;
    }
    let eq = line.find('=')?;
    let rhs = line[eq + 1..].trim();
    let first = rhs.find('"')?;
    let rest = &rhs[first + 1..];
    let last = rest.find('"')?;
    Some(rest[..last].to_string())
}

#[cfg(target_os = "linux")]
fn linux_list_video_devices(flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    use std::{fs, path::Path};

    let base = Path::new("/sys/class/video4linux");
    let mut idxs: Vec<u32> = Vec::new();

    let rd = match fs::read_dir(base) {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };

    for e in rd.flatten() {
        let name = match e.file_name().to_str().map(|s| s.to_string()) {
            Some(v) => v,
            None => continue,
        };
        if !name.starts_with("video") {
            continue;
        }
        let idx: u32 = match name[5..].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        idxs.push(idx);
    }

    idxs.sort_unstable();

    if flags.debug || flags.verbose >= 2 {
        eprintln!("INFO: found video nodes: {idxs:?}");
    }

    let mut out = Vec::new();
    for idx in idxs {
        let devnode = format!("/dev/video{idx}");
        if !Path::new(&devnode).exists() {
            continue;
        }

        let sys = base.join(format!("video{idx}"));
        let name = fs::read_to_string(sys.join("name"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| devnode.clone());

        let is_usb = linux_is_usb(&sys);

        out.push(DeviceInfo {
            id: format!("file:{devnode}"),
            name,
            is_usb,
        });
    }

    Ok(out)
}

#[cfg(target_os = "linux")]
fn linux_is_usb(sys_video: &std::path::Path) -> bool {
    use std::fs;
    let dev = sys_video.join("device");
    let link = fs::read_link(&dev).ok();
    if let Some(p) = link {
        let s = p.to_string_lossy().to_lowercase();
        if s.contains("usb") {
            return true;
        }
    }

    let uevent = fs::read_to_string(dev.join("uevent")).ok().unwrap_or_default();
    let u = uevent.to_lowercase();
    u.contains("usb")
}

#[cfg(target_os = "windows")]
fn windows_list_video_devices(flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    use std::process::Command;

    if flags.debug || flags.verbose >= 2 {
        eprintln!("INFO: listing Windows DirectShow devices via ffmpeg");
    }

    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-f", "dshow", "-list_devices", "true", "-i", "dummy"])
        .output()
        .map_err(|e| CameraError::driver("running ffmpeg -list_devices", e))?;

    let stderr = String::from_utf8_lossy(&out.stderr);
    Ok(parse_dshow_video_devices(&stderr))
}

#[cfg(target_os = "windows")]
fn parse_dshow_video_devices(s: &str) -> Vec<DeviceInfo> {
    let mut out = Vec::new();
    let mut in_video = false;

    for line in s.lines() {
        if line.contains("DirectShow video devices") {
            in_video = true;
            continue;
        }
        if in_video && line.contains("DirectShow audio devices") {
            break;
        }
        if !in_video {
            continue;
        }

        if let Some(name) = extract_dshow_quoted_name(line) {
            let n = name.to_lowercase();
            let is_usb = n.contains("usb") || n.contains("webcam") || n.contains("capture");
            out.push(DeviceInfo {
                id: format!("dshow:video={}", name),
                name,
                is_usb,
            });
        }
    }

    out
}

#[cfg(target_os = "windows")]
fn extract_dshow_quoted_name(line: &str) -> Option<String> {
    let l = line.trim();
    if !l.starts_with('"') {
        return None;
    }
    let rest = &l[1..];
    let end = rest.find('"')?;
    let name = &rest[..end];
    if name.is_empty() { None } else { Some(name.to_string()) }
}
