// This is free and unencumbered software released into the public domain.

use crate::shared::{CameraError, Frame};
use std::{
    any::Any,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::JoinHandle,
    time::Duration,
};

pub type FrameSink = Arc<dyn Fn(Frame) + Send + Sync + 'static>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraBackend {
    Android,
    Avf,
    Dshow,
    V4l2,
    Ffmpeg,
}

#[derive(Debug)]
pub enum CameraEvent {
    Started {
        backend: CameraBackend,
    },
    Stopped {
        backend: CameraBackend,
    },
    FrameDropped {
        backend: CameraBackend,
    },
    Warning {
        backend: CameraBackend,
        message: String,
    },
    Error {
        backend: CameraBackend,
        error: CameraError,
    },
}

pub enum FrameMsg {
    Frame(Frame),
    Stop,
}

pub struct Dispatcher {
    tx: SyncSender<FrameMsg>,
    sinks: Arc<RwLock<Vec<FrameSink>>>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Dispatcher {
    pub fn new(
        capacity: usize,
        backend: CameraBackend,
        events_tx: SyncSender<CameraEvent>,
    ) -> Self {
        let (tx, rx) = sync_channel::<FrameMsg>(capacity.max(1));
        let sinks: Arc<RwLock<Vec<FrameSink>>> = Arc::new(RwLock::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));

        let sinks2 = Arc::clone(&sinks);
        let stop2 = Arc::clone(&stop);

        let join = std::thread::spawn(move || {
            let _ = events_tx.try_send(CameraEvent::Started { backend });

            while !stop2.load(Ordering::Relaxed) {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(FrameMsg::Frame(frame)) => {
                        if let Ok(list) = sinks2.read() {
                            for s in list.iter() {
                                (s)(frame.clone());
                            }
                        }
                    },
                    Ok(FrameMsg::Stop) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }

            let _ = events_tx.try_send(CameraEvent::Stopped { backend });
        });

        Self {
            tx,
            sinks,
            stop,
            join: Some(join),
        }
    }

    pub fn sender(&self) -> SyncSender<FrameMsg> {
        self.tx.clone()
    }

    pub fn add_sink(&self, sink: FrameSink) {
        if let Ok(mut g) = self.sinks.write() {
            g.push(sink);
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.tx.try_send(FrameMsg::Stop);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub trait CameraDriver: Send {
    fn backend(&self) -> CameraBackend;
    fn start(&mut self) -> Result<(), CameraError>;
    fn stop(&mut self) -> Result<(), CameraError> {
        Ok(())
    }
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct Camera {
    driver: Box<dyn CameraDriver>,
    dispatcher: Dispatcher,
    events_rx: Receiver<CameraEvent>,
}

impl Camera {
    #[cfg_attr(
        not(any(
            all(
                feature = "ffmpeg",
                any(target_os = "macos", target_os = "linux", target_os = "windows")
            ),
            all(feature = "avf", any(target_os = "macos", target_os = "ios")),
            all(feature = "android", target_os = "android"),
            all(feature = "dshow", target_os = "windows"),
            all(feature = "v4l2", target_os = "linux"),
        )),
        allow(dead_code)
    )]
    pub(crate) fn new(
        driver: Box<dyn CameraDriver>,
        dispatcher: Dispatcher,
        events_rx: Receiver<CameraEvent>,
    ) -> Self {
        Self {
            driver,
            dispatcher,
            events_rx,
        }
    }

    pub fn backend(&self) -> CameraBackend {
        self.driver.backend()
    }

    pub fn add_sink(&self, sink: FrameSink) {
        self.dispatcher.add_sink(sink);
    }

    pub fn events(&self) -> &Receiver<CameraEvent> {
        &self.events_rx
    }

    pub fn start(&mut self) -> Result<(), CameraError> {
        self.driver.start()
    }

    pub fn stop(&mut self) -> Result<(), CameraError> {
        let r = self.driver.stop();
        self.dispatcher.stop();
        r
    }

    pub fn driver_as<T: 'static>(&self) -> Option<&T> {
        self.driver.as_any().downcast_ref::<T>()
    }

    pub fn driver_as_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.driver.as_any_mut().downcast_mut::<T>()
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub fn report_drop(events_tx: &SyncSender<CameraEvent>, backend: CameraBackend) {
    let _ = events_tx.try_send(CameraEvent::FrameDropped { backend });
}

pub fn try_send_frame(
    frame_tx: &SyncSender<FrameMsg>,
    events_tx: &SyncSender<CameraEvent>,
    backend: CameraBackend,
    frame: Frame,
) {
    match frame_tx.try_send(FrameMsg::Frame(frame)) {
        Ok(()) => {},
        Err(TrySendError::Full(_)) => report_drop(events_tx, backend),
        Err(TrySendError::Disconnected(_)) => {
            let _ = events_tx.try_send(CameraEvent::Error {
                backend,
                error: CameraError::Closed,
            });
        },
    }
}
