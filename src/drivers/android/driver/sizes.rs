// This is free and unencumbered software released into the public domain.

use core::ffi::c_char;
use core::ptr::null_mut;

use ndk_sys as ndk;

use crate::drivers::android::ndk::CameraManager;

#[inline]
fn tag_available_stream_configurations() -> u32 {
    ndk::acamera_metadata_tag::ACAMERA_SCALER_AVAILABLE_STREAM_CONFIGURATIONS.0 as u32
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
    if candidates.is_empty() || desired_w <= 0 || desired_h <= 0 {
        return candidates.first().copied();
    }

    let dw = desired_w as f64;
    let dh = desired_h as f64;
    let desired_aspect = dw / dh;
    let desired_area = (desired_w as i64) * (desired_h as i64);

    let mut best: Option<((i32, i32), f64, i64)> = None;

    for &(w, h) in candidates {
        let wf = w as f64;
        let hf = h as f64;

        let a1 = wf / hf;
        let a2 = hf / wf;
        let aspect_diff = (a1 - desired_aspect).abs().min((a2 - desired_aspect).abs());

        let area = (w as i64) * (h as i64);
        let area_diff = (area - desired_area).abs();

        match best {
            None => best = Some(((w, h), aspect_diff, area_diff)),
            Some((_, best_ad, best_area)) => {
                if aspect_diff < best_ad || (aspect_diff == best_ad && area_diff < best_area) {
                    best = Some(((w, h), aspect_diff, area_diff));
                }
            },
        }
    }

    best.map(|(s, _, _)| s)
}
