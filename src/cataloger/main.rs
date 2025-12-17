// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-cataloger requires the 'std' feature");

use asimov_camera_module::cli;
use asimov_camera_module::shared::CameraError;
use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraInfo, FrameFormat, RequestedFormat, RequestedFormatType, Resolution},
};
use serde_json::json;
use std::{collections::HashMap, error::Error as StdError, fmt::Debug};

/// asimov-camera-cataloger
#[derive(Debug, Parser)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    /// Output format: human-readable text or JSONL.
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

    let exit_code = match run_cataloger(&options) {
        Ok(()) => EX_OK,
        Err(err) => cli::handle_error(&err, &options.flags),
    };

    Ok(exit_code)
}

fn run_cataloger(options: &Options) -> Result<(), CameraError> {
    cli::info_user(&options.flags, "enumerating camera devices");

    #[cfg(target_os = "macos")]
    nokhwa::nokhwa_initialize(|_| ());

    let backend = nokhwa::native_api_backend().ok_or_else(|| CameraError::NoDriver)?;

    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

    let devices = nokhwa::query(backend)
        .map_err(|e| CameraError::driver("querying camera devices via nokhwa", e))?;

    if devices.is_empty() {
        cli::warn_user(&options.flags, "no camera devices found");
        return Ok(());
    }

    for dev in devices {
        let index = dev.index().clone();
        let logical_id = logical_camera_id(&index);

        let formats: Option<HashMap<Resolution, Vec<u32>>> =
            match nokhwa::Camera::new(index.clone(), requested.clone()) {
                Ok(mut camera) => match camera.compatible_list_by_resolution(FrameFormat::RAWRGB) {
                    Ok(map) => Some(map),
                    Err(e) => {
                        if options.flags.debug || options.flags.verbose >= 2 {
                            eprintln!("WARN: failed to query compatible formats: {e}");
                        } else if options.flags.verbose >= 1 {
                            eprintln!("WARN: failed to query compatible formats");
                        }
                        None
                    },
                },
                Err(e) => {
                    if options.flags.debug || options.flags.verbose >= 2 {
                        eprintln!("WARN: failed to open camera for format query: {e}");
                    } else if options.flags.verbose >= 1 {
                        eprintln!("WARN: failed to open camera for format query");
                    }
                    None
                },
            };

        match options.output {
            OutputFormat::Text => {
                print_device_text(&logical_id, &dev, formats.as_ref(), &options.flags);
            },
            OutputFormat::Jsonl => {
                print_device_jsonl(&logical_id, &dev, formats.as_ref());
            },
        }
    }

    Ok(())
}

fn logical_camera_id(index: &nokhwa::utils::CameraIndex) -> String {
    format!("file:/dev/video{}", index)
}

fn print_device_text<F: Debug>(
    logical_id: &str,
    dev: &CameraInfo,
    formats: Option<&HashMap<Resolution, Vec<F>>>,
    flags: &StandardOptions,
) {
    println!("{logical_id}: {}", dev.human_name());

    if !(flags.debug || flags.verbose >= 1) {
        return;
    }

    println!("\t{}", dev.description());
    println!("\t{}", dev.misc());

    if let Some(formats) = formats {
        println!("\tAvailable formats:");
        for (resolution, rates) in formats {
            println!(
                "\t\tResolution {}x{}",
                resolution.width(),
                resolution.height()
            );
            for rate in rates {
                println!("\t\t\tFrame rate: {:?}", rate);
            }
        }
    }
}

fn print_device_jsonl<F: Debug>(
    logical_id: &str,
    dev: &CameraInfo,
    formats: Option<&HashMap<Resolution, Vec<F>>>,
) {
    let formats_json: Vec<_> = formats
        .into_iter()
        .flat_map(|map| map.iter())
        .map(|(res, rates)| {
            let rates_json: Vec<_> = rates
                .iter()
                .map(|r| json!({ "value": format!("{:?}", r) }))
                .collect();

            json!({
                "width": res.width(),
                "height": res.height(),
                "frame_rates": rates_json,
            })
        })
        .collect();

    let info = json!({
        "id": logical_id,
        "name": dev.human_name(),
        "description": dev.description(),
        "misc": dev.misc(),
        "formats": formats_json,
    });

    println!("{info}");
}
