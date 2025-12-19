// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-cataloger requires the 'std' feature");

use asimov_camera_module::shared::CameraError;
use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use serde_json::json;
use std::error::Error as StdError;

#[derive(Debug, Parser)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    #[arg(value_name = "FORMAT", short = 'o', long = "output", value_enum, default_value = "text")]
    output: OutputFormat,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Jsonl,
}

#[derive(Clone, Debug)]
struct DeviceInfo {
    id: String,
    name: String,
    is_usb: bool,
}

pub fn main() -> Result<SysexitsError, Box<dyn StdError>> {
    asimov_module::dotenv().ok();
    let args = asimov_module::args_os()?;
    let options = Options::parse_from(args);

    if options.flags.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(EX_OK);
    }

    if options.flags.license {
        print!("{}", include_str!("../../UNLICENSE"));
        return Ok(EX_OK);
    }

    #[cfg(feature = "tracing")]
    asimov_module::init_tracing_subscriber(&options.flags).expect("failed to initialize logging");

    let exit_code = match run_cataloger(&options) {
        Ok(()) => EX_OK,
        Err(err) => handle_error(&err, &options.flags),
    };

    Ok(exit_code)
}

fn run_cataloger(options: &Options) -> Result<(), CameraError> {
    if options.flags.debug || options.flags.verbose >= 1 {
        eprintln!("INFO: enumerating camera devices");
    }

    let mut devices = list_video_devices(&options.flags)?;
    if devices.is_empty() {
        if options.flags.debug || options.flags.verbose >= 1 {
            eprintln!("WARN: no camera devices found");
        }
        return Ok(());
    }

    devices.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.name.cmp(&b.name)));

    for d in devices {
        match options.output {
            OutputFormat::Text => {
                if d.is_usb {
                    println!("{}: {} [usb]", d.id, d.name);
                } else {
                    println!("{}: {}", d.id, d.name);
                }
            }
            OutputFormat::Jsonl => {
                println!("{}", json!({ "id": d.id, "name": d.name, "usb": d.is_usb }));
            }
        }
    }

    Ok(())
}

fn handle_error(err: &CameraError, flags: &StandardOptions) -> SysexitsError {
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

    match err {
        CameraError::NoDriver => EX_UNAVAILABLE,
        CameraError::NoCamera => EX_USAGE,
        CameraError::NotConfigured => EX_CONFIG,
        CameraError::InvalidConfig(_) => EX_USAGE,
        CameraError::Unsupported(_) => EX_UNAVAILABLE,
        CameraError::DriverError { .. } => EX_SOFTWARE,
        CameraError::Other(_) => EX_SOFTWARE,
        _ => EX_SOFTWARE,
    }
}

#[cfg(target_os = "macos")]
fn list_video_devices(_flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    let devs = macos_avfoundation_video_devices()?
        .into_iter()
        .map(|d| DeviceInfo { id: format!("avf:{}", d.index), name: d.name, is_usb: false })
        .collect::<Vec<_>>();
    Ok(devs)
}

#[cfg(target_os = "linux")]
fn list_video_devices(_flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    linux_video4linux_devices()
}

#[cfg(target_os = "windows")]
fn list_video_devices(_flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    windows_dshow_devices()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn list_video_devices(_flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct AvfVideoDevice {
    index: u32,
    name: String,
}

#[cfg(target_os = "macos")]
fn macos_avfoundation_video_devices() -> Result<Vec<AvfVideoDevice>, CameraError> {
    use std::process::Command;

    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
        .map_err(|e| CameraError::driver("running ffmpeg -list_devices", e))?;

    let stderr = String::from_utf8_lossy(&out.stderr);
    Ok(parse_avfoundation_video_devices(&stderr).unwrap_or_default())
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

        devices.push(AvfVideoDevice { index: idx, name: name.to_string() });
    }

    if devices.is_empty() { None } else { Some(devices) }
}

#[cfg(target_os = "linux")]
fn linux_video4linux_devices() -> Result<Vec<DeviceInfo>, CameraError> {
    use std::fs;
    use std::path::Path;

    let base = Path::new("/sys/class/video4linux");
    let mut entries: Vec<u32> = Vec::new();

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
        entries.push(idx);
    }

    entries.sort_unstable();

    let mut out = Vec::new();
    for idx in entries {
        let devnode = format!("/dev/video{idx}");
        if !Path::new(&devnode).exists() {
            continue;
        }
        out.push(DeviceInfo { id: format!("file:{devnode}"), name: devnode, is_usb: false });
    }

    Ok(out)
}

#[cfg(target_os = "windows")]
fn windows_dshow_devices() -> Result<Vec<DeviceInfo>, CameraError> {
    use std::process::Command;

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
            let is_usb = n.contains("usb") || n.contains("capture") || n.contains("webcam") || n.contains("camera");
            out.push(DeviceInfo { id: format!("dshow:video={}", name), name, is_usb });
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
