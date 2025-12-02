use asimov_camera_module::shared::{CameraConfig, CameraDriver, FrameCallback, open_camera};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::{thread, time::Duration};

fn main() {
    let device = String::new();
    let width: u32 = 640;
    let height: u32 = 480;
    let fps: f64 = 30.0;

    let config = CameraConfig::new(device.clone(), width, height, fps);

    let frame_counter = Arc::new(AtomicU64::new(0));

    let callback: FrameCallback = {
        let frame_counter = frame_counter.clone();
        Box::new(move |frame| {
            let n = frame_counter.fetch_add(1, Ordering::Relaxed) + 1;

            println!(
                "frame #{n}: {}x{} stride={} bytes={} format={:?}",
                frame.width,
                frame.height,
                frame.stride,
                frame.data.len(),
                frame.pixel_format,
            );

            if n >= 50 {
                println!("Got {n} frames, exiting.");
                std::process::exit(0);
            }
        })
    };

    let mut driver: Box<dyn CameraDriver> =
        open_camera("", config, callback).expect("failed to open camera via open_camera");

    driver.start().expect("failed to start camera driver");
    println!("open_camera() driver started, waiting for frames...");

    loop {
        thread::sleep(Duration::from_millis(100));
    }
}
