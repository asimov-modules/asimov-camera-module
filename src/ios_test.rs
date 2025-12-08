use crate::shared::{CameraConfig, CameraDriver, FrameCallback, open_camera};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::thread;
use std::time::Duration;

#[unsafe(no_mangle)]
pub extern "C" fn asimov_camera_avf_test() {
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
                "iOS AVF frame #{n}: {}x{} stride={} bytes={} format={:?}",
                frame.width,
                frame.height,
                frame.stride,
                frame.data.len(),
                frame.pixel_format,
            );
        })
    };

    let mut driver: Box<dyn CameraDriver> = match open_camera("", config, callback) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to open camera via open_camera(): {e}");
            return;
        }
    };

    if let Err(e) = driver.start() {
        eprintln!("failed to start camera driver: {e}");
        return;
    }

    println!("asímov iOS AVF test: driver started, waiting for frames...");

    // 👇 keep the driver alive for a bit so frames can arrive
    thread::sleep(Duration::from_secs(5));

    println!("stopping driver after 5 seconds...");
    let _ = driver.stop();
}
