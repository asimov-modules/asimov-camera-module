// This is free and unencumbered software released into the public domain.

use crate::{CameraError, CaptureSettings, DeviceInfo};

#[derive(Clone, Debug)]
pub struct DriverConfig {
    pub device: DeviceInfo,
    pub settings: CaptureSettings,
}

impl DriverConfig {
    pub fn normalized(mut self) -> Self {
        self.settings = self.settings.normalized();
        self
    }

    pub fn validate(&self) -> Result<(), CameraError> {
        let id = self.device.id().trim();
        if id.is_empty() {
            return Err(CameraError::invalid_config("driver device id is empty"));
        }
        self.settings.validate()
    }
}
