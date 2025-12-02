// src/avf-test/main.rs
// This is free and unencumbered software released into the public domain.

use asimov_camera_module::shared::{CameraConfig, CameraDriver, FrameCallback, open_camera};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::{thread, time::Duration};

fn main() {
    // --- Hardcoded test config ---
    // For AVF: config.device is what AvfCameraDriver::find_device() uses.
    // Empty string => default camera.
    let device = String::new(); // or "FaceTime HD Camera".to_string()
    let width: u32 = 640;
    let height: u32 = 480;
    let fps: f64 = 30.0;

    let config = CameraConfig::new(device.clone(), width, height, fps);

    // --- Frame callback: just print some info about the bytes ---
    let frame_counter = Arc::new(AtomicU64::new(0));

    let callback: FrameCallback = {
        let frame_counter = frame_counter.clone();
        Box::new(move |data, w, h, stride| {
            let n = frame_counter.fetch_add(1, Ordering::Relaxed) + 1;
            println!(
                "frame #{n}: {}x{} stride={} bytes={}",
                w,
                h,
                stride,
                data.len()
            );

            if n >= 50 {
                println!("Got {n} frames, exiting.");
                std::process::exit(0);
            }
        })
    };

    // --- Open driver through open_camera() ---
    //
    // NOTE: For AVF, the input_url is currently unused in your AvfCameraDriver::open,
    // but open_camera requires it. Passing an empty string is fine.
    //
    // IMPORTANT: to actually hit AVF:
    //   - build WITHOUT the `ffmpeg` feature, or
    //   - change open_camera so ffmpeg isn't the first branch.
    let mut driver: Box<dyn CameraDriver> =
        open_camera("", config, callback).expect("failed to open camera via open_camera");

    // --- Start streaming ---
    driver.start().expect("failed to start camera driver");
    println!("open_camera() driver started, waiting for frames...");

    // Keep main thread alive; frames arrive on the AVF dispatch queue
    loop {
        thread::sleep(Duration::from_millis(100));
    }
}
