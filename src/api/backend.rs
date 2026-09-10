// This is free and unencumbered software released into the public domain.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraBackend {
    Android,
    Avf,
    Ffmpeg,
}
