// This is free and unencumbered software released into the public domain.

use derive_more::{Display, Error, From};

#[derive(Clone, Debug, Display, Error, From)]
pub enum CameraError {
    NoCamera,
}
