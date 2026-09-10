// This is free and unencumbered software released into the public domain.

use crate::{CameraError, DeviceInfo, DeviceKind};

use core::ffi::{c_char, c_int};

use super::ndk::CameraManager;

fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let cstr = unsafe { core::ffi::CStr::from_ptr(ptr) };
    cstr.to_string_lossy().to_string()
}

#[inline]
fn tag_lens_facing() -> u32 {
    ndk_sys::acamera_metadata_tag::ACAMERA_LENS_FACING.0 as u32
}

fn classify_kind(mgr: &CameraManager, camera_id: *const c_char) -> DeviceKind {
    use ndk_sys as ndk;

    unsafe {
        let mut meta: *mut ndk::ACameraMetadata = core::ptr::null_mut();
        let status = mgr.get_characteristics(camera_id, &mut meta);

        if status != ndk::camera_status_t::ACAMERA_OK || meta.is_null() {
            return DeviceKind::Unknown;
        }

        let mut entry: ndk::ACameraMetadata_const_entry = core::mem::zeroed();
        let st = ndk::ACameraMetadata_getConstEntry(
            meta as *const ndk::ACameraMetadata,
            tag_lens_facing(),
            &mut entry,
        );

        ndk::ACameraMetadata_free(meta);

        if st != ndk::camera_status_t::ACAMERA_OK {
            return DeviceKind::Unknown;
        }

        if entry.data.u8_.is_null() {
            return DeviceKind::Unknown;
        }

        let facing_u8: u8 = *entry.data.u8_;
        match facing_u8 as c_int {
            0 => DeviceKind::Front,
            1 => DeviceKind::Back,
            2 => DeviceKind::External,
            _ => DeviceKind::Unknown,
        }
    }
}

pub fn list_video_devices() -> Result<Vec<DeviceInfo>, CameraError> {
    let mgr = CameraManager::new()
        .map_err(|st| CameraError::driver("android: ACameraManager_create", st))?;

    let id_list = mgr
        .list_camera_ids()
        .map_err(|st| CameraError::driver("android: ACameraManager_getCameraIdList", st))?;

    let n = id_list.len();
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        let id_ptr = id_list.id_ptr(i);
        if id_ptr.is_null() {
            continue;
        }

        let id = cstr_to_string(id_ptr);
        let kind = classify_kind(&mgr, id_ptr);

        out.push(DeviceInfo::new(id.clone(), id, kind));
    }

    Ok(out)
}
