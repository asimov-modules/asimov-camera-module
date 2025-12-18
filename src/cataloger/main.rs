// This is free and unencumbered software released into the public domain.

#[cfg(not(feature = "std"))]
compile_error!("asimov-camera-cataloger requires the 'std' feature");

use asimov_camera_module::cli;
use asimov_module::SysexitsError::{self, *};
use clap::Parser;
use clientele::StandardOptions;
use serde_json::json;
use std::error::Error as StdError;

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

fn run_cataloger(options: &Options) -> Result<(), asimov_camera_module::shared::CameraError> {
    cli::info_user(&options.flags, "enumerating camera devices");

    #[cfg(target_os = "macos")]
    {
        let devices = cli::macos_avfoundation_video_devices().unwrap_or_default();

        if devices.is_empty() {
            cli::warn_user(&options.flags, "no camera devices found");
            return Ok(());
        }

        for d in devices {
            let id = format!("file:/dev/video{}", d.index);
            match options.output {
                OutputFormat::Text => {
                    println!("{id}: {}", d.name);
                },
                OutputFormat::Jsonl => {
                    println!("{}", json!({ "id": id, "name": d.name }));
                },
            }
        }

        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        cli::warn_user(
            &options.flags,
            "cataloger is best-effort on this platform (improve later)",
        );
        Ok(())
    }
}
