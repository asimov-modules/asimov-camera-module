// This is free and unencumbered software released into the public domain.

use crate::{CameraError, DeviceInfo, DeviceKind};

pub fn list_video_devices() -> Result<Vec<DeviceInfo>, CameraError> {
    use objc2::rc::Retained;
    use objc2_av_foundation::{
        AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDevicePosition,
        AVCaptureDeviceType, AVCaptureDeviceTypeBuiltInWideAngleCamera,
        AVCaptureDeviceTypeExternal, AVMediaTypeVideo,
    };
    use objc2_foundation::{NSArray, NSString};

    unsafe {
        let built_in = AVCaptureDeviceTypeBuiltInWideAngleCamera;
        let external = AVCaptureDeviceTypeExternal;

        let device_types: Retained<NSArray<AVCaptureDeviceType>> =
            NSArray::from_slice(&[built_in, external]);

        let media = AVMediaTypeVideo.expect("AVMediaTypeVideo is unavailable");

        let session: Retained<AVCaptureDeviceDiscoverySession> =
            AVCaptureDeviceDiscoverySession::discoverySessionWithDeviceTypes_mediaType_position(
                &device_types,
                Some(media),
                AVCaptureDevicePosition::Unspecified,
            );

        let devices: Retained<NSArray<AVCaptureDevice>> = session.devices();
        let mut out = Vec::with_capacity(devices.count() as usize);

        for i in 0..devices.count() {
            let dev = devices.objectAtIndex(i);

            let uid: Retained<NSString> = dev.uniqueID();
            let id = uid.to_string();

            let lname: Retained<NSString> = dev.localizedName();
            let name = lname.to_string();

            let dtype: Retained<AVCaptureDeviceType> = dev.deviceType();
            let pos = dev.position();

            let is_external = *dtype == *external;

            let kind = if is_external {
                DeviceKind::External
            } else {
                match pos {
                    AVCaptureDevicePosition::Front => DeviceKind::Front,
                    AVCaptureDevicePosition::Back => DeviceKind::Back,
                    _ => {
                        if *dtype == *built_in {
                            DeviceKind::Front
                        } else {
                            DeviceKind::Unknown
                        }
                    },
                }
            };

            out.push(DeviceInfo::new(id.clone(), name, kind));
        }

        Ok(out)
    }
}
