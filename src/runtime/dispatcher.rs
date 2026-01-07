// This is free and unencumbered software released into the public domain.

#![allow(dead_code, unused_imports)]

use crossbeam_channel as ch;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

/// A consumer of dispatched items (e.g., preview callback, recorder, processing pipeline).
pub type SinkFn<T> = dyn Fn(T) + Send + Sync + 'static;

/// Optional hook invoked when a sink panics.
///
/// This is intentionally not given the panic payload by default to avoid accidental
/// heavy formatting/logging in a hot path. If you need details, you can adapt this
/// signature to take `&(dyn Any + Send)` and do sampling/formatting externally.
pub type SinkPanicHook = dyn Fn(SinkPanicInfo) + Send + Sync + 'static;

/// Metadata about a sink panic for observability.
#[derive(Clone, Copy, Debug)]
pub struct SinkPanicInfo {
    /// Total number of sink panics observed by this dispatcher (monotonic).
    pub panic_count: u64,
    /// Whether this panic was sampled (true) or suppressed (false).
    pub sampled: bool,
}

/// Configuration options for `Dispatcher`.
#[derive(Clone)]
pub struct DispatcherOptions {
    /// Thread name for the dispatcher worker thread.
    pub thread_name: String,

    /// Stop behavior:
    /// - `None`: block until stop signal is delivered (best effort to join immediately).
    /// - `Some(d)`: attempt to deliver stop signal within `d`, then join anyway.
    pub stop_timeout: Option<Duration>,

    /// Optional hook to observe sink panics.
    ///
    /// The hook is called on every sink panic, with `SinkPanicInfo.sampled` indicating
    /// whether this panic is part of the sampling stream (true) or suppressed (false).
    /// This allows you to keep ultra-light counters on every panic, while only doing
    /// heavier work (logging, reporting) on sampled panics.
    pub on_sink_panic: Option<Arc<SinkPanicHook>>,

    /// Panic sampling: mark `sampled=true` for the first panic and then every Nth panic.
    /// Set to 1 to mark every panic as sampled. Must be >= 1.
    pub panic_sample_every: u64,
}

impl Default for DispatcherOptions {
    fn default() -> Self {
        Self {
            thread_name: "asimov-dispatcher".to_string(),
            stop_timeout: Some(Duration::from_millis(250)),
            on_sink_panic: None,
            panic_sample_every: 128,
        }
    }
}

/// A lightweight fan-out dispatcher:
/// - One bounded input queue
/// - One worker thread
/// - A hot-swappable sink list
///
/// Notes:
/// - The returned `Sender<T>` can be cloned to support multiple producers.
/// - Sinks are invoked sequentially in the worker thread. If you need sink isolation
///   (e.g., heavy processing must not affect preview), use per-sink queues/workers.
pub struct Dispatcher<T: Clone + Send + Sync + 'static> {
    sinks: Arc<RwLock<Vec<Arc<SinkFn<T>>>>>,
    stop_tx: ch::Sender<()>,
    join_handle: Option<std::thread::JoinHandle<()>>,
    stopping: Arc<AtomicBool>,

    panic_count: Arc<AtomicU64>,
    opts: DispatcherOptions,
}

impl<T: Clone + Send + Sync + 'static> Dispatcher<T> {
    /// Create a dispatcher with default options.
    pub fn new(capacity: usize) -> (Self, ch::Sender<T>) {
        Self::new_with_options(capacity, DispatcherOptions::default())
    }

    /// Create a dispatcher with custom options.
    pub fn new_with_options(capacity: usize, mut opts: DispatcherOptions) -> (Self, ch::Sender<T>) {
        if opts.panic_sample_every == 0 {
            opts.panic_sample_every = 1;
        }

        let (data_tx, data_rx) = ch::bounded::<T>(capacity.max(1));
        let (stop_tx, stop_rx) = ch::bounded::<()>(1);

        let sinks = Arc::new(RwLock::new(Vec::<Arc<SinkFn<T>>>::new()));
        let sinks_thread = Arc::clone(&sinks);

        let stopping = Arc::new(AtomicBool::new(false));
        let stopping_thread = Arc::clone(&stopping);

        let panic_count = Arc::new(AtomicU64::new(0));
        let panic_count_thread = Arc::clone(&panic_count);

        let on_sink_panic = opts.on_sink_panic.clone();
        let panic_sample_every = opts.panic_sample_every;

        let thread_name = opts.thread_name.clone();
        let join_handle = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                loop {
                    ch::select! {
                        recv(stop_rx) -> _ => break,
                        recv(data_rx) -> msg => {
                            let item = match msg {
                                Ok(v) => v,
                                Err(_) => break, // all senders dropped
                            };

                            if stopping_thread.load(Ordering::Relaxed) {
                                continue;
                            }

                            let active_sinks: Vec<Arc<SinkFn<T>>> = match sinks_thread.read() {
                                Ok(lock) => lock.clone(),
                                Err(poisoned) => poisoned.into_inner().clone(),
                            };

                            for sink in active_sinks {
                                let item_for_sink = item.clone();
                                let res = catch_unwind(AssertUnwindSafe(|| {
                                    (*sink)(item_for_sink);
                                }));

                                if res.is_err() {
                                    let n = panic_count_thread.fetch_add(1, Ordering::Relaxed) + 1;
                                    let sampled = n == 1 || (n % panic_sample_every == 0);

                                    if let Some(hook) = on_sink_panic.as_ref() {
                                        // Called on every panic; consumer can branch on `sampled`
                                        // to keep heavy work (logs/reports) off the hot path.
                                        hook(SinkPanicInfo { panic_count: n, sampled });
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .expect("failed to spawn dispatcher worker thread");

        (
            Self {
                sinks,
                stop_tx,
                join_handle: Some(join_handle),
                stopping,
                panic_count,
                opts,
            },
            data_tx,
        )
    }

    /// Add a sink while the dispatcher is running.
    pub fn add_sink(&self, sink: impl Fn(T) + Send + Sync + 'static) {
        let sink = Arc::new(sink) as Arc<SinkFn<T>>;
        match self.sinks.write() {
            Ok(mut lock) => lock.push(sink),
            Err(poisoned) => poisoned.into_inner().push(sink),
        }
    }

    /// Return the current number of sinks.
    pub fn sink_count(&self) -> usize {
        match self.sinks.read() {
            Ok(lock) => lock.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        }
    }

    /// Return the total number of sink panics observed.
    pub fn sink_panic_count(&self) -> u64 {
        self.panic_count.load(Ordering::Relaxed)
    }

    /// Stop the dispatcher and join the worker thread.
    ///
    /// This function is idempotent.
    pub fn stop(&mut self) {
        if self.stopping.swap(true, Ordering::Relaxed) {
            return;
        }

        match self.opts.stop_timeout {
            Some(d) => {
                let _ = self.stop_tx.send_timeout((), d);
            },
            None => {
                let _ = self.stop_tx.send(());
            },
        }

        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Drop for Dispatcher<T> {
    fn drop(&mut self) {
        self.stop();
    }
}
