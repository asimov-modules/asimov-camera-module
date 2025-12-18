// This is free and unencumbered software released into the public domain.

//! CLI helpers (error reporting, verbosity handling).
//!
//! This module must compile even when the crate feature `cli` is disabled,
//! because the library is built in non-CLI configurations.

#[cfg(feature = "cli")]
use crate::shared::CameraError;

#[cfg(feature = "cli")]
use asimov_module::SysexitsError::{self, *};

#[cfg(feature = "cli")]
use clientele::StandardOptions;

#[cfg(feature = "cli")]
pub fn handle_error(err: &CameraError, flags: &StandardOptions) -> SysexitsError {
    #[cfg(feature = "tracing")]
    {
        use asimov_module::tracing::{debug, error};

        error!(target: "asimov_camera_module", %err, "camera command failed");

        if flags.debug || flags.verbose >= 2 {
            debug!(target: "asimov_camera_module", ?err, "detailed error");
        }
    }

    report_error(err, flags);
    map_error_to_sysexit(err)
}

#[cfg(feature = "cli")]
pub fn info_user(flags: &StandardOptions, msg: &str) {
    if flags.debug || flags.verbose >= 1 {
        eprintln!("INFO: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::info!(target: "asimov_camera_module", "{msg}");
}

#[cfg(feature = "cli")]
pub fn warn_user(flags: &StandardOptions, msg: &str) {
    if flags.debug || flags.verbose >= 1 {
        eprintln!("WARN: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::warn!(target: "asimov_camera_module", "{msg}");
}

#[cfg(feature = "cli")]
pub fn warn_user_with_error(flags: &StandardOptions, msg: &str, error: &dyn std::error::Error) {
    if flags.debug || flags.verbose >= 2 {
        eprintln!("WARN: {msg}: {error}");
    } else if flags.verbose >= 1 {
        eprintln!("WARN: {msg}");
    }

    #[cfg(feature = "tracing")]
    asimov_module::tracing::warn!(target: "asimov_camera_module", error = %error, "{msg}");
}

#[cfg(feature = "cli")]
fn report_error(err: &CameraError, flags: &StandardOptions) {
    use std::error::Error as _;
    use std::io::Write;

    let mut stderr = std::io::stderr();
    let _ = writeln!(stderr, "ERROR: {err}");

    if flags.debug || flags.verbose >= 2 {
        let mut source = err.source();
        while let Some(cause) = source {
            let _ = writeln!(stderr, "  Caused by: {}", cause);
            source = cause.source();
        }
    }
}

#[cfg(feature = "cli")]
fn map_error_to_sysexit(err: &CameraError) -> SysexitsError {
    match err {
        CameraError::NoDriver => EX_UNAVAILABLE,
        CameraError::NoCamera => EX_USAGE,
        CameraError::NotConfigured => EX_CONFIG,
        CameraError::InvalidConfig(_) => EX_USAGE,
        CameraError::Unsupported(_) => EX_UNAVAILABLE,
        CameraError::DriverError { .. } => EX_SOFTWARE,
        CameraError::Other(_) => EX_SOFTWARE,
    }
}

// When `cli` is disabled, keep the module linkable without exposing CLI-only types.
#[cfg(not(feature = "cli"))]
#[inline]
pub fn info_user(_msg: &str) {}

#[cfg(not(feature = "cli"))]
#[inline]
pub fn warn_user(_msg: &str) {}

// This is free and unencumbered software released into the public domain.

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
pub struct AvfVideoDevice {
    pub index: u32,
    pub name: String,
}

#[cfg(target_os = "macos")]
pub fn macos_avfoundation_video_devices() -> Result<Vec<AvfVideoDevice>, crate::shared::CameraError>
{
    use std::process::Command;

    let out = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-f",
            "avfoundation",
            "-list_devices",
            "true",
            "-i",
            "",
        ])
        .output()
        .map_err(|e| crate::shared::CameraError::driver("running ffmpeg -list_devices", e))?;

    // ffmpeg writes device list to stderr
    let stderr = String::from_utf8_lossy(&out.stderr);
    Ok(parse_avfoundation_video_devices(&stderr).unwrap_or_default())
}

#[cfg(target_os = "macos")]
pub fn macos_preferred_device_id_usb_first() -> Result<Option<String>, crate::shared::CameraError> {
    let usb_names = macos_usb_product_names().unwrap_or_default();
    if usb_names.is_empty() {
        return Ok(None);
    }

    let devices = macos_avfoundation_video_devices()?;
    for dev in devices {
        if usb_names
            .iter()
            .any(|usb| contains_case_insensitive(&dev.name, usb))
        {
            return Ok(Some(format!("file:/dev/video{}", dev.index)));
        }
    }

    Ok(None)
}

#[cfg(target_os = "macos")]
fn macos_usb_product_names() -> Option<Vec<String>> {
    use std::process::Command;

    let out = Command::new("ioreg")
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

    let names = dedup_preserve_order(names);
    if names.is_empty() { None } else { Some(names) }
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

        let line = line.trim();

        // Example: "[0] FaceTime HD Camera"
        let Some(bracket_start) = line.find('[') else {
            continue;
        };
        let Some(bracket_end) = line[bracket_start + 1..].find(']') else {
            continue;
        };

        let idx_str = &line[bracket_start + 1..bracket_start + 1 + bracket_end];
        let idx: u32 = match idx_str.trim().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let rest = line[bracket_start + 1 + bracket_end + 1..].trim();
        if rest.is_empty() {
            continue;
        }

        devices.push(AvfVideoDevice {
            index: idx,
            name: rest.to_string(),
        });
    }

    if devices.is_empty() {
        None
    } else {
        Some(devices)
    }
}

#[cfg(target_os = "macos")]
fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
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

#[cfg(target_os = "macos")]
fn dedup_preserve_order(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v {
        if !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    }
    out
}
