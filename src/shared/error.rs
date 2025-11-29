// This is free and unencumbered software released into the public domain.

#[derive(Debug, Display, Error, From)]
pub enum CameraError {
    NoCamera,
}
