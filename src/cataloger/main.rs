// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-cataloger requires the 'std' feature");

use asimov_camera_module::{cli, shared::CameraError};
use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use serde_json::json;
use std::error::Error as StdError;

#[derive(Debug, Parser)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    #[arg(
        value_name = "FORMAT",
        short = 'o',
        long = "output",
        value_enum,
        default_value = "text"
    )]
    output: OutputFormat,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Jsonl,
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

    let mut devices = cli::list_video_devices(&options.flags)?;
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
            },
            OutputFormat::Jsonl => {
                println!("{}", json!({ "id": d.id, "name": d.name, "usb": d.is_usb }));
            },
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
