// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-reader requires the 'std' feature");

use asimov_camera_module::{
    shared::{open_camera, CameraConfig, Frame, PixelFormat},
};
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

    #[arg(long)]
    list_devices: bool,
}

#[derive(Clone, Debug)]
struct DeviceInfo {
    id: String,
    name: String,
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
        Err(err) => {
            eprintln!("ERROR: {err}");
            EX_SOFTWARE
        }
    };

    Ok(exit_code)
}

fn run_reader(opts: &Options) -> Result<(), asimov_camera_module::shared::CameraError> {
    if opts.list_devices {
        let devices = list_video_devices(&opts.flags)?;
        for d in devices {
            println!("{}: {}", d.id, d.name);
        }
        return Ok(());
    }

    let quit = Arc::new(AtomicBool::new(false));
    {
        let quit2 = Arc::clone(&quit);
        ctrlc::set_handler(move || {
            quit2.store(true, Ordering::SeqCst);
        })
            .map_err(|e| asimov_camera_module::shared::CameraError::other(format!("{e}")))?;
    }

    let (width, height) = opts.size;
    let fps = opts.frequency.max(0.1);
    let min_interval = Duration::from_secs_f64(1.0 / fps);

    let hasher = (opts.debounce > 0).then(|| HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher());

    let device_id = match opts.device.clone() {
        Some(d) => d,
        None => auto_select_device(&opts.flags)?.unwrap_or_else(default_device_for_platform),
    };

    let config = CameraConfig::new(width, height, fps).with_device(device_id.clone());

    let last_emit = Arc::new(Mutex::new(Instant::now()));
    let last_hash: Arc<Mutex<Option<image_hasher::ImageHash>>> = Arc::new(Mutex::new(None));

    let quit_cb = Arc::clone(&quit);
    let last_emit_cb = Arc::clone(&last_emit);
    let last_hash_cb = Arc::clone(&last_hash);
    let debounce_level = opts.debounce;
    let device_id_cb = device_id.clone();

    let callback = Arc::new(move |frame: Frame| {
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
            if let Some(img_buffer) = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_raw(
                frame.width,
                frame.height,
                frame.data.to_vec(),
            ) {
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
            id: Some(format!("{device_id_cb}#{ts_secs}")),
            width: Some(frame.width as _),
            height: Some(frame.height as _),
            data: frame.data.to_vec(),
            source: Some(device_id_cb.clone()),
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

    let mut cam = open_camera("", config)?;
    cam.add_sink(callback);
    cam.start()?;

    while !quit.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = cam.stop();
    Ok(())
}

fn default_device_for_platform() -> String {
    #[cfg(target_os = "macos")]
    {
        "avf:0".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "file:/dev/video0".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "dshow:video=default".to_string()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "file:/dev/video0".to_string()
    }
}

fn auto_select_device(flags: &StandardOptions) -> Result<Option<String>, asimov_camera_module::shared::CameraError> {
    let devices = list_video_devices(flags)?;
    if devices.is_empty() {
        return Ok(None);
    }

    #[cfg(target_os = "macos")]
    {
        let usb_names = macos_usb_product_names().unwrap_or_default();
        if !usb_names.is_empty() {
            for d in &devices {
                if usb_names.iter().any(|u| contains_case_insensitive(&d.name, u)) {
                    return Ok(Some(d.id.clone()));
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        for d in &devices {
            if contains_case_insensitive(&d.name, "usb") {
                return Ok(Some(d.id.clone()));
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        for d in &devices {
            if contains_case_insensitive(&d.name, "usb") {
                return Ok(Some(d.id.clone()));
            }
        }
    }

    Ok(Some(devices[0].id.clone()))
}

fn list_video_devices(_flags: &StandardOptions) -> Result<Vec<DeviceInfo>, asimov_camera_module::shared::CameraError> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .map_err(|e| asimov_camera_module::shared::CameraError::driver("running ffmpeg -list_devices", e))?;

        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut devices = Vec::new();
        let mut in_video = false;

        for line in stderr.lines() {
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
            let idx_str = tail[1..end_bracket].trim();
            let idx: u32 = match idx_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let name = tail[end_bracket + 1..].trim();
            if name.is_empty() {
                continue;
            }
            devices.push(DeviceInfo {
                id: format!("avf:{idx}"),
                name: name.to_string(),
            });
        }

        return Ok(devices);
    }

    #[cfg(target_os = "linux")]
    {
        let out = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-f", "v4l2", "-list_devices", "true", "-i", "dummy"])
            .output()
            .unwrap_or_else(|_| Default::default());
        let _ = out;
        return Ok(vec![DeviceInfo { id: "file:/dev/video0".to_string(), name: "/dev/video0".to_string() }]);
    }

    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .output()
            .unwrap_or_else(|_| Default::default());
        let stderr = String::from_utf8_lossy(&out.stderr);
        let mut devices = Vec::new();

        for line in stderr.lines() {
            let s = line.trim();
            if let Some(name) = s.strip_prefix("\"").and_then(|r| r.strip_suffix("\"")) {
                if !name.is_empty() {
                    devices.push(DeviceInfo {
                        id: format!("dshow:video={name}"),
                        name: name.to_string(),
                    });
                }
            }
        }

        if devices.is_empty() {
            devices.push(DeviceInfo { id: "dshow:video=default".to_string(), name: "default".to_string() });
        }

        return Ok(devices);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(Vec::new())
    }
}

#[cfg(target_os = "macos")]
fn macos_usb_product_names() -> Option<Vec<String>> {
    let out = std::process::Command::new("ioreg").args(["-p", "IOUSB", "-l"]).output().ok()?;
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

    let mut out = Vec::new();
    for s in names {
        if !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    }

    if out.is_empty() { None } else { Some(out) }
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

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
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
