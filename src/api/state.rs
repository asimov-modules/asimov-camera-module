// This is free and unencumbered software released into the public domain.

/// `Camera` lifecycle state.
///
/// ```text
/// Idle ⇄ Running ⇄ Stopped
///   \       |         /
///    \      v        /
///     `--> Closed <-'
/// ```
///
/// `Closed` is terminal and reachable from any other state; every other
/// transition is reversible via `start()`/`stop()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraState {
    /// Opened, never started.
    Idle,
    /// Actively capturing.
    Running,
    /// Paused: was `Running`, capture stopped but the underlying
    /// device/session is still held — `start()` resumes cheaply, without
    /// repeating device enumeration/negotiation.
    Stopped,
    /// Terminally released. No further operation is valid.
    Closed,
}
