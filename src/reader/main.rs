// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-reader requires the 'std' feature");

use asimov_camera_module::shared::{open_camera, CameraConfig, CameraError, Frame, FrameSink, PixelFormat};
use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use image_hasher::{HashAlg, HasherConfig};
use know::traits::ToJsonLd;
use std::{
    error::Error as StdError,
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Parser)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    #[arg(long)]
    device: Option<String>,

    #[arg(short, long = "size", value_parser = parse_dimensions, default_value = "640x480")]
    size: (u32, u32),

    #[arg(short, long, value_parser = parse_frequency, default_value = "30")]
    frequency: f64,

    #[clap(short = 'D', long, action = clap::ArgAction::Count)]
    debounce: u8,
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

    let exit_code = match run_reader(&options) {
        Ok(()) => EX_OK,
        Err(err) => handle_error(&err, &options.flags),
    };

    Ok(exit_code)
}

fn run_reader(opts: &Options) -> Result<(), CameraError> {
    info_user(&opts.flags, "starting camera reader");

    let quit = Arc::new(AtomicBool::new(false));
    {
        let quit2 = Arc::clone(&quit);
        ctrlc::set_handler(move || {
            quit2.store(true, Ordering::SeqCst);
        })
            .map_err(|e| CameraError::other(format!("{e}")))?;
    }

    let (width, height) = opts.size;
    let fps = opts.frequency.max(0.000_001);
    let min_interval = Duration::from_secs_f64(1.0 / fps);

    let hasher = if opts.debounce > 0 {
        Some(HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher())
    } else {
        None
    };

    let selected = auto_select_device_usb_first(&opts.flags, opts.device.clone())?
        .unwrap_or_else(default_device_fallback);

    if opts.flags.debug || opts.flags.verbose >= 1 {
        info_user(&opts.flags, &format!("selected device: {selected}"));
    }

    let mut config = CameraConfig::new(width, height, fps);
    config = config.with_device(selected.clone());
    config = config.with_diagnostics(opts.flags.debug || opts.flags.verbose >= 3);

    let last_emit = Arc::new(Mutex::new(Instant::now()));
    let last_hash: Arc<Mutex<Option<image_hasher::ImageHash>>> = Arc::new(Mutex::new(None));

    let quit_cb = Arc::clone(&quit);
    let last_emit_cb = Arc::clone(&last_emit);
    let last_hash_cb = Arc::clone(&last_hash);
    let selected_cb = selected.clone();
    let debounce_level = opts.debounce;

    let sink: FrameSink = Arc::new(move |frame: Frame| {
        if quit_cb.load(Ordering::SeqCst) {
            return;
        }

        if frame.pixel_format != PixelFormat::Rgb8 {
            return;
        }

        {
            let mut guard = last_emit_cb.lock().unwrap_or_else(|p| p.into_inner());
            let now = Instant::now();
            if now.duration_since(*guard) < min_interval {
                return;
            }
            *guard = now;
        }

        if let Some(ref hasher) = hasher {
            let raw = frame.data.to_vec();
            if let Some(img_buffer) =
                image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(frame.width, frame.height, raw)
            {
                let img_data = image::DynamicImage::ImageRgb8(img_buffer);
                let hash = hasher.hash_image(&img_data);

                let mut prev = last_hash_cb.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(ref mut prev_hash) = *prev {
                    if hash.dist(prev_hash) < debounce_level as u32 {
                        return;
                    }
                    *prev_hash = hash;
                } else {
                    *prev = Some(hash);
                }
            }
        }

        let ts_secs: u64 = if frame.timestamp_ns != 0 {
            frame.timestamp_ns / 1_000_000_000
        } else {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::from_secs(0))
                .as_secs()
        };

        let img = know::classes::Image {
            id: Some(format!("{selected_cb}#{ts_secs}")),
            width: Some(frame.width as _),
            height: Some(frame.height as _),
            data: frame.data.to_vec(),
            source: Some(selected_cb.clone()),
        };

        let json = match img.to_jsonld() {
            Ok(v) => v,
            Err(_) => return,
        };

        let mut out = io::stdout().lock();
        if let Err(err) = writeln!(&mut out, "{json}") {
            if err.kind() == io::ErrorKind::BrokenPipe {
                quit_cb.store(true, Ordering::SeqCst);
            }
        }
    });

    let mut camera = open_camera(selected.clone(), config)?;
    camera.add_sink(sink);
    camera.start()?;

    while !quit.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = camera.stop();
    Ok(())
}

fn info_user(flags: &StandardOptions, msg: &str) {
    if flags.debug || flags.verbose >= 1 {
        eprintln!("INFO: {msg}");
    }
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

#[derive(Clone, Debug)]
struct DeviceInfo {
    id: String,
    is_usb: bool,
}

#[cfg(target_os = "macos")]
fn auto_select_device_usb_first(_flags: &StandardOptions, explicit: Option<String>) -> Result<Option<String>, CameraError> {
    if let Some(s) = explicit {
        let t = s.trim();
        if !t.is_empty() {
            return Ok(Some(t.to_string()));
        }
    }

    if let Ok(Some(id)) = macos_preferred_device_id_usb_first() {
        return Ok(Some(id));
    }

    Ok(Some("avf:0".to_string()))
}

#[cfg(not(target_os = "macos"))]
fn auto_select_device_usb_first(flags: &StandardOptions, explicit: Option<String>) -> Result<Option<String>, CameraError> {
    if let Some(s) = explicit {
        let t = s.trim();
        if !t.is_empty() {
            return Ok(Some(t.to_string()));
        }
    }

    let devices = list_video_devices(flags)?;
    if devices.is_empty() {
        return Ok(None);
    }

    if let Some(usb) = devices.iter().find(|d| d.is_usb) {
        return Ok(Some(usb.id.clone()));
    }

    Ok(Some(devices[0].id.clone()))
}

#[cfg(target_os = "macos")]
fn list_video_devices(_flags: &StandardOptions) -> Result<Vec<DeviceInfo>, CameraError> {
    let devs = macos_avfoundation_video_devices()?
        .into_iter()
        .map(|d| DeviceInfo { id: format!("avf:{}", d.index), is_usb: false })
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

fn default_device_fallback() -> String {
    #[cfg(target_os = "macos")]
    {
        "avf:0".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "dshow:video=default".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "file:/dev/video0".to_string()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "".to_string()
    }
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
fn macos_preferred_device_id_usb_first() -> Result<Option<String>, CameraError> {
    let usb_names = macos_usb_product_names().unwrap_or_default();
    if usb_names.is_empty() {
        return Ok(None);
    }

    let devices = macos_avfoundation_video_devices()?;
    for dev in devices {
        if usb_names.iter().any(|usb| contains_case_insensitive(&dev.name, usb)) {
            return Ok(Some(format!("avf:{}", dev.index)));
        }
    }

    Ok(None)
}

#[cfg(target_os = "macos")]
fn macos_usb_product_names() -> Option<Vec<String>> {
    use std::process::Command;

    let out = Command::new("ioreg").args(["-p", "IOUSB", "-l"]).output().ok()?;
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
        let is_usb = linux_device_is_usb(base.join(format!("video{idx}")));
        out.push(DeviceInfo { id: format!("file:{devnode}"), is_usb });
    }

    Ok(out)
}

#[cfg(target_os = "linux")]
fn linux_device_is_usb(sys_entry: std::path::PathBuf) -> bool {
    use std::fs;

    let device_link = sys_entry.join("device");
    let mut p = match fs::read_link(&device_link) {
        Ok(p) => p,
        Err(_) => return false,
    };

    if !p.is_absolute() {
        p = sys_entry.join("device").join(p);
    }

    if let Ok(canon) = p.canonicalize() {
        p = canon;
    }

    let s = p.to_string_lossy();
    s.contains("/usb") || s.contains("\\usb")
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
            out.push(DeviceInfo { id: format!("dshow:video={}", name), is_usb });
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

fn parse_dimensions(s: &str) -> Result<(u32, u32), String> {
    let s = s.trim().replace('×', "x");
    let parts: Vec<&str> = s.split('x').map(|t| t.trim()).collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!("Invalid format '{s}'. Use WxH (e.g., 1920x1080)"));
    }

    let width: u32 = parts[0].parse().map_err(|_| format!("Invalid width: {}", parts[0]))?;
    let height: u32 = parts[1].parse().map_err(|_| format!("Invalid height: {}", parts[1]))?;

    if !(160..=7680).contains(&width) {
        return Err(format!("Width {width} is out of reasonable range (160-7680)"));
    }
    if !(120..=4320).contains(&height) {
        return Err(format!("Height {height} is out of reasonable range (120-4320)"));
    }

    Ok((width, height))
}

fn parse_frequency(s: &str) -> Result<f64, String> {
    let freq: f64 = s.parse().map_err(|_| format!("Invalid frequency: {s}"))?;

    if freq <= 0.0 {
        return Err("Frequency must be positive".to_string());
    }
    if freq > 240.0 {
        return Err(format!("Frequency {freq} Hz exceeds reasonable maximum (240 Hz)"));
    }
    if freq < 0.1 {
        return Err(format!("Frequency {freq} Hz is below reasonable minimum (0.1 Hz)"));
    }

    Ok(freq)
}
