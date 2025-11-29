// This is free and unencumbered software released into the public domain.

use crate::shared::CameraDriver;

#[derive(Clone, Debug)]
pub struct V4l2CameraDriver {}

impl CameraDriver for V4l2CameraDriver {}
