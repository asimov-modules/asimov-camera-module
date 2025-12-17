// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-reader requires the 'std' feature");

use asimov_camera_module::{
    cli,
    shared::{CameraConfig, Frame, PixelFormat, open_camera},
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
        Err(err) => cli::handle_error(&err, &options.flags),
    };

    Ok(exit_code)
}

fn run_reader(opts: &Options) -> Result<(), asimov_camera_module::shared::CameraError> {
    cli::info_user(&opts.flags, "starting camera reader");

    let quit = Arc::new(AtomicBool::new(false));
    {
        let quit = quit.clone();
        ctrlc::set_handler(move || {
            quit.store(true, Ordering::SeqCst);
        })
        .map_err(|e| {
            asimov_camera_module::shared::CameraError::other(format!(
                "failed to install Ctrl+C handler: {e}"
            ))
        })?;
    }

    let (width, height) = opts.size;
    let fps = opts.frequency;
    let min_interval = Duration::from_secs_f64(1.0 / fps);

    let hasher = if opts.debounce > 0 {
        Some(HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher())
    } else {
        None
    };

    let last_emit = Arc::new(Mutex::new(Instant::now()));
    let last_hash: Arc<Mutex<Option<image_hasher::ImageHash>>> = Arc::new(Mutex::new(None));

    let device_id = opts.device.clone();
    let config = CameraConfig::new(device_id.clone(), width, height, fps);

    let quit_cb = quit.clone();
    let last_emit_cb = last_emit.clone();
    let last_hash_cb = last_hash.clone();
    let debounce_level = opts.debounce;

    let callback = Box::new(move |frame: Frame| {
        if quit_cb.load(Ordering::SeqCst) {
            return;
        }

        if frame.pixel_format != PixelFormat::Rgb8 {
            return;
        }

        {
            let mut guard = last_emit_cb
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let now = Instant::now();
            if now.duration_since(*guard) < min_interval {
                return;
            }
            *guard = now;
        }

        // Debounce (best-effort, never panic)
        if let Some(ref hasher) = hasher {
            if let Some(img_buffer) = image::ImageBuffer::<image::Rgb<u8>, _>::from_raw(
                frame.width,
                frame.height,
                frame.data.clone(),
            ) {
                let img_data = image::DynamicImage::ImageRgb8(img_buffer);
                let hash = hasher.hash_image(&img_data);

                let mut prev = last_hash_cb
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());

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
            id: Some(format!("{device_id}#{ts_secs}")),
            width: Some(frame.width as _),
            height: Some(frame.height as _),
            data: frame.data.clone(),
            source: Some(device_id.clone()),
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

    let mut driver = open_camera("", config, callback)?;

    driver.start()?;

    while !quit.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = driver.stop();

    #[cfg(feature = "tracing")]
    asimov_module::tracing::info!(target: "asimov_camera_module::reader", "camera reader exiting");

    Ok(())
}

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
