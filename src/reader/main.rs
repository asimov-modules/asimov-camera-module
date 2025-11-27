// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-reader requires the 'std' feature");

use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use image_hasher::{HashAlg, HasherConfig};
use know::traits::ToJsonLd;
use std::{
    error::Error,
    io::{self, Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// asimov-camera-reader
#[derive(Debug, Parser)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    /// Input camera device.
    ///
    /// Linux:
    ///   "0"                 -> /dev/video0
    ///   "file:/dev/video0"  -> /dev/video0
    ///
    /// macOS:
    ///   "0"                 -> first AVFoundation camera
    ///   "1"                 -> second camera, etc.
    ///
    /// Windows:
    ///   'Integrated Camera' -> video=Integrated Camera
    ///   'video=default'     -> video=default
    ///
    /// If omitted, a per-OS default is used.
    device: Option<String>,

    /// Desired dimensions in WxH format (e.g. 1920x1080)
    #[arg(short, long = "size", value_parser = parse_dimensions, default_value = "640x480")]
    size: (u32, u32),

    /// Sampling frequency in Hz (frames per second)
    #[arg(short, long, value_parser = parse_frequency, default_value = "30")]
    frequency: f64,

    /// Debounce level. Repeat for stricter debouncing.
    ///
    /// Debouncing is implemented as a similarity comparison to the previously emitted image's hash.
    #[clap(short = 'D', long, action = clap::ArgAction::Count)]
    debounce: u8,
}

pub fn main() -> Result<SysexitsError, Box<dyn Error>> {
    // Load environment variables from `.env`:
    asimov_module::dotenv().ok();

    // Expand wildcards and @argfiles:
    let args = asimov_module::args_os()?;

    // Parse command-line options:
    let options = Options::parse_from(args);

    // Handle the `--version` flag:
    if options.flags.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(EX_OK);
    }

    // Handle the `--license` flag:
    if options.flags.license {
        print!("{}", include_str!("../../UNLICENSE"));
        return Ok(EX_OK);
    }

    // Configure logging & tracing:
    #[cfg(feature = "tracing")]
    asimov_module::init_tracing_subscriber(&options.flags).expect("failed to initialize logging");

    let quit = Arc::new(AtomicBool::new(false));
    let child_holder: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));

    {
        let quit = quit.clone();
        let child_holder = child_holder.clone();
        ctrlc::set_handler(move || {
            quit.store(true, Ordering::SeqCst);
            if let Ok(mut guard) = child_holder.lock() {
                if let Some(child) = guard.as_mut() {
                    let _ = child.kill();
                }
            }
        })?;
    }

    // Resolve device and normalize per OS:
    let device = options.device.clone().unwrap_or_else(default_device_for_os);
    let input_device = get_input_device(&device)?;

    let (width, height) = options.size;
    let fps = options.frequency;

    let min_interval = Duration::from_secs_f64(1.0 / fps);
    let fps_s = fps.to_string();

    let mut last_hash: Option<image_hasher::ImageHash> = None;
    let hasher = if options.debounce > 0 {
        Some(HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher())
    } else {
        None
    };

    let mut ffargs: Vec<String> = vec![
        "-hide_banner".into(),
        "-f".into(),
        get_ffmpeg_format()?.into(),
        "-loglevel".into(),
        "error".into(),
        "-video_size".into(),
        format!("{}x{}", width, height),
        "-framerate".into(),
        fps_s.clone(),
    ];

    // macOS: preselect a supported input pixel format to avoid AVFoundation pixel-format spam.
    // logs showed `0rgb` among supported formats; pick that first.
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
    asimov_module::tracing::info!(
        target: "asimov_camera_module::reader",
        device = %device,
        input_device = %input_device,
        width = width,
        height = height,
        fps = fps,
        debounce = options.debounce,
        ffmpeg_args = ?ffargs,
        "spawning ffmpeg"
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.args(ffargs)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let child = cmd.spawn()?;
    *child_holder.lock().unwrap() = Some(child);
    let mut child = child_holder.lock().unwrap().take().unwrap();

    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let mut reader = io::BufReader::new(stdout);

    let frame_size = (width * height * 3) as usize;
    let mut buffer = vec![0u8; frame_size];

    let mut out = io::stdout().lock();
    let mut last_emit = Instant::now();

    while !quit.load(Ordering::SeqCst) {
        match reader.read_exact(&mut buffer) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        if quit.load(Ordering::SeqCst) {
            break;
        }

        let now = Instant::now();
        if now.duration_since(last_emit) < min_interval {
            continue;
        }
        last_emit = now;

        if let Some(ref hasher) = hasher {
            let img_buffer =
                image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(width, height, buffer.clone())
                    .ok_or("Failed to create image buffer")?;
            let img_data = image::DynamicImage::ImageRgb8(img_buffer);
            let hash = hasher.hash_image(&img_data);

            if let Some(ref mut prev_hash) = last_hash {
                let dist = hash.dist(prev_hash);
                if dist < options.debounce as u32 {
                    continue;
                }
                *prev_hash = hash;
            } else {
                last_hash = Some(hash);
            };
        };

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let img = know::classes::Image {
            id: Some(format!("{}#{}", &device, ts)),
            width: Some(width as _),
            height: Some(height as _),
            data: buffer.clone(),
            source: Some(device.clone()),
        };

        let json = img.to_jsonld().map_err(|e| -> Box<dyn Error> {
            format!("failed to convert image to JSON-LD: {e}").into()
        })?;

        match writeln!(&mut out, "{json}") {
            Ok(_) => (),
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => break,
            Err(err) => return Err(err.into()),
        }
    }

    let _ = child.kill();

    #[cfg(feature = "tracing")]
    asimov_module::tracing::info!(
        target: "asimov_camera_module::reader",
        "camera reader exiting"
    );

    Ok(EX_OK)
}

fn default_device_for_os() -> String {
    #[cfg(target_os = "linux")]
    {
        // First v4l2 camera
        return "file:/dev/video0".to_string();
    }

    #[cfg(target_os = "macos")]
    {
        // First AVFoundation camera index
        return "0".to_string();
    }

    #[cfg(target_os = "windows")]
    {
        // Default dshow video device
        return "video=default".to_string();
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        return String::from("");
    }
}

#[cfg(target_os = "macos")]
fn get_ffmpeg_format() -> Result<&'static str, Box<dyn Error>> {
    Ok("avfoundation")
}

#[cfg(target_os = "linux")]
fn get_ffmpeg_format() -> Result<&'static str, Box<dyn Error>> {
    Ok("v4l2")
}

#[cfg(target_os = "windows")]
fn get_ffmpeg_format() -> Result<&'static str, Box<dyn Error>> {
    Ok("dshow")
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn get_ffmpeg_format() -> Result<&'static str, Box<dyn Error>> {
    Err("asimov-camera-reader: unsupported OS for camera input".into())
}

fn get_input_device(device: &str) -> Result<String, Box<dyn Error>> {
    #[cfg(target_os = "macos")]
    {
        // On macOS, ffmpeg avfoundation expects an index ("0", "1", "0:0", etc.).
        //
        // We accept:
        //   "0"                      -> "0"
        //   "1"                      -> "1"
        //   "file:/dev/video1"       -> "1"
        //   "/dev/video2"            -> "2"   (Linux-style habit, normalized)
        //
        if let Some(rest) = device.strip_prefix("file:/dev/video") {
            Ok(rest.to_string())
        } else if let Some(rest) = device.strip_prefix("/dev/video") {
            Ok(rest.to_string())
        } else {
            Ok(device.to_string())
        }
    }

    #[cfg(target_os = "linux")]
    {
        // v4l2: typically /dev/videoN
        //
        // Accepted:
        //   "0"                 -> /dev/video0
        //   "file:/dev/video0"  -> /dev/video0
        //   "/dev/video2"       -> /dev/video2
        if device.chars().all(|c| c.is_numeric()) {
            Ok(format!("/dev/video{}", device))
        } else {
            Ok(device.strip_prefix("file:").unwrap_or(device).to_string())
        }
    }

    #[cfg(target_os = "windows")]
    {
        // dshow: expects something like:
        //   video="Integrated Camera"
        //   video=default
        //
        // We normalize inputs:
        //   - if it already starts with "video=" -> use as is
        //   - if "default" (any case)          -> video=default
        //   - otherwise                         -> video=<device>
        if device.to_lowercase().starts_with("video=") {
            Ok(device.to_string())
        } else if device.eq_ignore_ascii_case("default") {
            Ok("video=default".to_string())
        } else {
            Ok(format!("video={}", device))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err("Unsupported OS".into())
    }
}

fn parse_dimensions(s: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid format '{}'. Use WxH (e.g., 1920x1080)", s));
    }

    let width: u32 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid width: {}", parts[0]))?;
    let height: u32 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid height: {}", parts[1]))?;

    if width < 160 || width > 7680 {
        return Err(format!(
            "Width {} is out of reasonable range (160-7680)",
            width
        ));
    }
    if height < 120 || height > 4320 {
        return Err(format!(
            "Height {} is out of reasonable range (120-4320)",
            height
        ));
    }

    Ok((width, height))
}

fn parse_frequency(s: &str) -> Result<f64, String> {
    let freq: f64 = s.parse().map_err(|_| format!("Invalid frequency: {}", s))?;

    if freq <= 0.0 {
        return Err("Frequency must be positive".to_string());
    }
    if freq > 240.0 {
        return Err(format!(
            "Frequency {} Hz exceeds reasonable maximum (240 Hz)",
            freq
        ));
    }
    if freq < 0.1 {
        return Err(format!(
            "Frequency {} Hz is below reasonable minimum (0.1 Hz)",
            freq
        ));
    }

    Ok(freq)
}
