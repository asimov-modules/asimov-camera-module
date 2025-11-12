// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-template-emitter requires the 'std' feature");

use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use image_hasher::{HashAlg, HasherConfig};
use know::traits::ToJsonLd;
use std::{
    error::Error,
    io::{self, Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// asimov-template-emitter
#[derive(Debug, Parser)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    /// Input camera device (e.g., "0" for /dev/video0 or device name)
    #[arg(default_value = "file:/dev/video0")]
    device: String,

    /// Desired dimensions in WxH format (e.g. 1920x1080)
    #[arg(short, long = "size", value_parser = parse_dimensions, default_value = "640x480")]
    size: (u32, u32),

    /// Sampling frequency in Hz (frames per second)
    #[arg(short, long, value_parser = parse_frequency, default_value = "30")]
    frequency: f64,

    /// Debounce level. Repeat for stricter debouncing.
    ///
    /// Deboucing is implemented as a similarity comparison to the previously emitted image's image
    /// hash.
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

    let input_device = get_input_device(&options.device)?;
    let (width, height) = options.size;
    let fps = options.frequency;

    let mut last_emit = Instant::now();
    let min_interval = Duration::from_secs_f64(1.0 / fps);

    let mut last_hash: Option<image_hasher::ImageHash> = None;
    let hasher = if options.debounce > 0 {
        Some(HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher())
    } else {
        None
    };

    // Build FFmpeg command
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-f",
        get_ffmpeg_format()?,
        "-loglevel",
        "error",
        "-video_size",
        &format!("{}x{}", width, height),
        "-framerate",
        "30",
        // &fps.to_string(),
        "-i",
        &input_device,
        "-preset",
        "veryfast",
        "-tune",
        "zerolatency",
        "-vf",
        "fps=30",
        "-pix_fmt",
        "rgb24",
        "-f",
        "rawvideo",
        "pipe:1",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;

    let mut reader = io::BufReader::new(stdout);
    let frame_size = (width * height * 3) as usize;
    let mut buffer = vec![0u8; frame_size];

    let mut stdout = io::stdout().lock();

    loop {
        // Read frame from FFmpeg
        match reader.read_exact(&mut buffer) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }

        // Check if we should emit based on frequency
        let now = Instant::now();
        if now.duration_since(last_emit) < min_interval {
            continue;
        }
        last_emit = now;

        // Apply debouncing if enabled
        if let Some(ref hasher) = hasher {
            // Convert raw RGB buffer to image
            let img_buffer =
                image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(width, height, buffer.clone())
                    .ok_or("Failed to create image buffer")?;
            let img_data = image::DynamicImage::ImageRgb8(img_buffer);
            let hash = hasher.hash_image(&img_data);

            if let Some(ref mut last_hash) = last_hash {
                let dist = hash.dist(last_hash);

                if dist < options.debounce as u32 {
                    continue;
                }

                *last_hash = hash;
            } else {
                last_hash = Some(hash)
            };
        };

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let img = know::classes::Image {
            id: Some(format!("{}#{}", &options.device, ts)),
            width: Some(width as _),
            height: Some(height as _),
            data: buffer.clone(),
            source: Some(options.device.clone()),
        };

        match writeln!(&mut stdout, "{}", img.to_jsonld().unwrap()) {
            Ok(_) => (),
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => break,
            Err(err) => return Err(err.into()),
        }
    }

    child.kill().ok();

    Ok(EX_OK)
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
    Err("Unsupported OS for camera input".into())
}

fn get_input_device(device: &str) -> Result<String, Box<dyn Error>> {
    #[cfg(target_os = "macos")]
    {
        // On macOS, device "0" typically refers to the first camera
        // You can list cameras with: ffmpeg -f avfoundation -list_devices true -i ""
        Ok(device
            .strip_prefix("file:/dev/video")
            .unwrap_or(device)
            .to_string())
    }

    #[cfg(target_os = "linux")]
    {
        if device.chars().all(|c| c.is_numeric()) {
            Ok(format!("/dev/video{}", device))
        } else {
            Ok(device.strip_prefix("file:").unwrap_or(device).to_string())
        }
    }

    #[cfg(target_os = "windows")]
    {
        Ok(device.to_string())
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
