// This is free and unencumbered software released into the public domain.

use core::ffi::c_char;
use core::ptr::null_mut;

use ndk_sys as ndk;

use crate::drivers::android::ndk::CameraManager;
use crate::drivers::resolution::pick_nearest_resolution;

#[inline]
fn tag_available_stream_configurations() -> u32 {
    ndk::acamera_metadata_tag::ACAMERA_SCALER_AVAILABLE_STREAM_CONFIGURATIONS.0 as u32
}

#[inline]
fn tag_sensor_orientation() -> u32 {
    ndk::acamera_metadata_tag::ACAMERA_SENSOR_ORIENTATION.0 as u32
}

/// Reads `ACAMERA_SENSOR_ORIENTATION`: the fixed angle (0/90/180/270)
/// between how the sensor is physically mounted and the device's natural
/// orientation. This is a per-device-model hardware constant, not the
/// phone's current rotation — the host app composes this with the live
/// device/display rotation when it renders preview.
pub(super) fn sensor_orientation(mgr: &CameraManager, camera_id: *const c_char) -> i32 {
    const FALLBACK_DEG: i32 = 90; // correct for most phones, wrong for e.g. tablets

    unsafe {
        let mut meta: *mut ndk::ACameraMetadata = null_mut();
        let st = mgr.get_characteristics(camera_id, &mut meta);

        if st != ndk::camera_status_t::ACAMERA_OK || meta.is_null() {
            return FALLBACK_DEG;
        }

        let mut entry: ndk::ACameraMetadata_const_entry = core::mem::zeroed();
        let st_e = ndk::ACameraMetadata_getConstEntry(
            meta as *const ndk::ACameraMetadata,
            tag_sensor_orientation(),
            &mut entry,
        );

        let deg = if st_e == ndk::camera_status_t::ACAMERA_OK && !entry.data.i32_.is_null() {
            *entry.data.i32_
        } else {
            FALLBACK_DEG
        };

        ndk::ACameraMetadata_free(meta);
        deg
    }
}

pub(super) fn list_supported_output_sizes(
    mgr: &CameraManager,
    camera_id: *const c_char,
    desired_format: i32,
) -> Vec<(i32, i32)> {
    unsafe {
        let mut meta: *mut ndk::ACameraMetadata = null_mut();
        let st = mgr.get_characteristics(camera_id, &mut meta);

        if st != ndk::camera_status_t::ACAMERA_OK || meta.is_null() {
            return Vec::new();
        }

        let mut entry: ndk::ACameraMetadata_const_entry = core::mem::zeroed();
        let st_e = ndk::ACameraMetadata_getConstEntry(
            meta as *const ndk::ACameraMetadata,
            tag_available_stream_configurations(),
            &mut entry,
        );

        if st_e != ndk::camera_status_t::ACAMERA_OK {
            ndk::ACameraMetadata_free(meta);
            return Vec::new();
        }

        let count = entry.count as usize;
        let tuples = count / 4;
        let mut out = Vec::new();

        if entry.data.i32_.is_null() || tuples == 0 {
            ndk::ACameraMetadata_free(meta);
            return Vec::new();
        }

        let base = entry.data.i32_;

        for i in 0..tuples {
            let off = i * 4;
            let fmt = *base.add(off + 0);
            let w = *base.add(off + 1);
            let h = *base.add(off + 2);
            let input = *base.add(off + 3);

            if input != 0 {
                continue;
            }
            if fmt != desired_format {
                continue;
            }
            if w > 0 && h > 0 {
                out.push((w, h));
            }
        }

        ndk::ACameraMetadata_free(meta);
        out.sort_unstable();
        out.dedup();
        out
    }
}

pub(super) fn pick_best_size(
    desired_w: i32,
    desired_h: i32,
    candidates: &[(i32, i32)],
) -> Option<(i32, i32)> {
    if desired_w <= 0 || desired_h <= 0 {
        return candidates.first().copied();
    }

    let candidates_u32: Vec<(u32, u32)> = candidates
        .iter()
        .map(|&(w, h)| (w as u32, h as u32))
        .collect();

    pick_nearest_resolution((desired_w as u32, desired_h as u32), &candidates_u32)
        .map(|(w, h)| (w as i32, h as i32))
}
