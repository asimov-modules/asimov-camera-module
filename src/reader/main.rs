// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-reader requires the 'std' feature");

use asimov_camera_module::{
    core::{self, Error, Result as CoreResult},
    shared::{CameraConfig, drivers::ffmpeg, open_camera},
};
use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use image_hasher::{HashAlg, HasherConfig};
use know::traits::ToJsonLd;
use std::{
    error::Error as StdError,
    io::{self, Read, Write},
    process::Child,
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
    /// Debouncing is implemented as a similarity comparison to the previously
    /// emitted image's perceptual hash.
    #[clap(short = 'D', long, action = clap::ArgAction::Count)]
    debounce: u8,
}

pub fn main() -> Result<SysexitsError, Box<dyn StdError>> {
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

    let exit_code = match run_reader(&options) {
        Ok(()) => EX_OK,
        Err(err) => core::handle_error(&err, &options.flags),
    };

    Ok(exit_code)
}

fn run_reader(opts: &Options) -> CoreResult<()> {
    core::info_user(&opts.flags, "starting camera reader");

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
        })
        .map_err(|e| Error::Other(format!("failed to install Ctrl+C handler: {e}")))?;
    }

    let (width, height) = opts.size;
    let fps = opts.frequency;

    let min_interval = Duration::from_secs_f64(1.0 / fps);

    let mut last_hash: Option<image_hasher::ImageHash> = None;
    let hasher = if opts.debounce > 0 {
        Some(HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher())
    } else {
        None
    };

    let config = CameraConfig::new(opts.device.clone(), width, height, fps);

    let child = ffmpeg::spawn_reader(&config)?;
    *child_holder
        .lock()
        .map_err(|_| Error::Other("failed to lock child holder".into()))? = Some(child);

    let mut child = child_holder
        .lock()
        .map_err(|_| Error::Other("failed to lock child holder".into()))?
        .take()
        .ok_or_else(|| Error::Other("ffmpeg child missing from holder".into()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Other("failed to open ffmpeg stdout".into()))?;
    let mut reader = io::BufReader::new(stdout);

    let frame_size = (width * height * 3) as usize;
    let mut buffer = vec![0u8; frame_size];

    let mut out = io::stdout().lock();
    let mut last_emit = Instant::now();

    while !quit.load(Ordering::SeqCst) {
        match reader.read_exact(&mut buffer) {
            Ok(_) => {},
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                return Err(Error::Io {
                    context: "reading ffmpeg output",
                    source: e,
                });
            },
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
                    .ok_or_else(|| {
                        Error::InvalidFrameSize(format!(
                            "failed to create image buffer {width}x{height}"
                        ))
                    })?;
            let img_data = image::DynamicImage::ImageRgb8(img_buffer);
            let hash = hasher.hash_image(&img_data);

            if let Some(ref mut prev_hash) = last_hash {
                let dist = hash.dist(prev_hash);
                if dist < opts.debounce as u32 {
                    continue;
                }
                *prev_hash = hash;
            } else {
                last_hash = Some(hash);
            }
        }

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        let img = know::classes::Image {
            id: Some(format!("{}#{}", &opts.device, ts)),
            width: Some(width as _),
            height: Some(height as _),
            data: buffer.clone(),
            source: Some(opts.device.clone()),
        };

        let json = img.to_jsonld().map_err(|e| Error::JsonLd(e.to_string()))?;

        match writeln!(&mut out, "{json}") {
            Ok(_) => {},
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => break,
            Err(err) => {
                return Err(Error::Io {
                    context: "writing JSON output",
                    source: err,
                });
            },
        }
    }

    let _ = child.kill();

    #[cfg(feature = "tracing")]
    asimov_module::tracing::info!(
        target: "asimov_camera_module::reader",
        "camera reader exiting"
    );

    Ok(())
}

/// Accepts "1920x1080", "1920×1080", with optional spaces. Validates reasonable ranges.
fn parse_dimensions(s: &str) -> Result<(u32, u32), String> {
    let s = s.trim().replace('×', "x");
    let parts: Vec<&str> = s.split('x').map(|t| t.trim()).collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!("Invalid format '{s}'. Use WxH (e.g., 1920x1080)"));
    }

    let width: u32 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid width: {}", parts[0]))?;
    let height: u32 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid height: {}", parts[1]))?;

    if !(160..=7680).contains(&width) {
        return Err(format!(
            "Width {width} is out of reasonable range (160-7680)"
        ));
    }
    if !(120..=4320).contains(&height) {
        return Err(format!(
            "Height {height} is out of reasonable range (120-4320)"
        ));
    }

    Ok((width, height))
}

fn parse_frequency(s: &str) -> Result<f64, String> {
    let freq: f64 = s.parse().map_err(|_| format!("Invalid frequency: {s}"))?;

    if freq <= 0.0 {
        return Err("Frequency must be positive".to_string());
    }
    if freq > 240.0 {
        return Err(format!(
            "Frequency {freq} Hz exceeds reasonable maximum (240 Hz)"
        ));
    }
    if freq < 0.1 {
        return Err(format!(
            "Frequency {freq} Hz is below reasonable minimum (0.1 Hz)"
        ));
    }

    Ok(freq)
}
