// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-reader requires the 'std' feature");

use asimov_camera_module::{
    cli,
    shared::{open_camera, CameraConfig, CameraError, CameraEvent, Frame, PixelFormat},
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

fn run_reader(opts: &Options) -> Result<(), CameraError> {
    if opts.list_devices {
        let mut devices = cli::list_video_devices(&opts.flags)?;
        devices.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.name.cmp(&b.name)));
        for d in devices {
            if d.is_usb {
                println!("{}: {} [usb]", d.id, d.name);
            } else {
                println!("{}: {}", d.id, d.name);
            }
        }
        return Ok(());
    }

    let verbose: u8 = opts.flags.verbose;
    let debug: bool = opts.flags.debug;

    let quit = Arc::new(AtomicBool::new(false));
    {
        let quit2 = Arc::clone(&quit);
        ctrlc::set_handler(move || {
            quit2.store(true, Ordering::SeqCst);
        })
            .map_err(|e| CameraError::other(format!("{e}")))?;
    }

    let (width, height) = opts.size;
    let fps = opts.frequency.max(0.1);
    let min_interval = Duration::from_secs_f64(1.0 / fps);

    let device_id = cli::auto_select_device(&opts.flags, opts.device.clone())?
        .unwrap_or_else(default_device_for_platform);

    let config = CameraConfig::new(width, height, fps)
        .with_device(device_id.clone())
        .with_diagnostics(debug || verbose >= 2);

    let last_emit = Arc::new(Mutex::new(Instant::now()));
    let last_hash: Arc<Mutex<Option<image_hasher::ImageHash>>> = Arc::new(Mutex::new(None));
    let hasher = (opts.debounce > 0).then(|| HasherConfig::new().hash_alg(HashAlg::Gradient).to_hasher());

    let quit_cb = Arc::clone(&quit);
    let last_emit_cb = Arc::clone(&last_emit);
    let last_hash_cb = Arc::clone(&last_hash);
    let debounce_level = opts.debounce;
    let device_id_cb = device_id.clone();

    let callback = Arc::new(move |frame: Frame| {
        if quit_cb.load(Ordering::SeqCst) {
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
            if frame.pixel_format == PixelFormat::Rgb8 {
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
        }

        let ts_ns: u64 = if frame.timestamp_ns != 0 {
            frame.timestamp_ns
        } else {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        };

        let img = know::classes::Image {
            id: Some(format!("{device_id_cb}#{ts_ns}")),
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

    if debug || verbose >= 1 {
        eprintln!("INFO: opening camera device={device_id}");
    }

    cam.start()?;

    while !quit.load(Ordering::SeqCst) {
        if debug || verbose >= 1 {
            drain_events(cam.events(), debug, verbose);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = cam.stop();
    Ok(())
}

fn drain_events(rx: &std::sync::mpsc::Receiver<CameraEvent>, debug: bool, verbose: u8) {
    loop {
        match rx.try_recv() {
            Ok(ev) => print_event(ev, debug, verbose),
            Err(std::sync::mpsc::TryRecvError::Empty) => break,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }
}

fn print_event(ev: CameraEvent, debug: bool, verbose: u8) {
    match ev {
        CameraEvent::Started { backend } => {
            if debug || verbose >= 1 {
                eprintln!("INFO: camera started ({backend:?})");
            }
        }
        CameraEvent::Stopped { backend } => {
            if debug || verbose >= 1 {
                eprintln!("INFO: camera stopped ({backend:?})");
            }
        }
        CameraEvent::FrameDropped { backend } => {
            if debug || verbose >= 2 {
                eprintln!("WARN: frame dropped ({backend:?})");
            }
        }
        CameraEvent::Warning { backend, message } => {
            if debug || verbose >= 1 {
                eprintln!("WARN: {backend:?}: {message}");
            }
        }
        CameraEvent::Error { backend, error } => {
            eprintln!("ERROR: {backend:?}: {error}");
        }
    }
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
