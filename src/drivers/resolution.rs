// This is free and unencumbered software released into the public domain.

/// Pick the candidate resolution closest to `desired`, minimizing aspect
/// ratio difference first and then area difference.
///
/// Unlike a naive nearest-match, this does **not** treat a transposed
/// candidate (e.g. 1920x1080 against a desired 1080x1920) as an equally good
/// match for a different aspect ratio — callers are expected to pass
/// `desired` already in the orientation they actually want (e.g. the real
/// preview surface's width/height), and silently swapping it here would
/// defeat that (this was a real bug: portrait preview windows could end up
/// matched against a landscape stream, causing visible stretching).
pub(crate) fn pick_nearest_resolution(
    desired: (u32, u32),
    candidates: &[(u32, u32)],
) -> Option<(u32, u32)> {
    if candidates.is_empty() {
        return None;
    }

    let (dw, dh) = desired;
    if dw == 0 || dh == 0 {
        return candidates.first().copied();
    }

    let desired_aspect = dw as f64 / dh as f64;
    let desired_area = (dw as i64) * (dh as i64);

    let score = |(w, h): (u32, u32)| -> (f64, i64) {
        let aspect_diff = (w as f64 / h as f64 - desired_aspect).abs();
        let area_diff = ((w as i64) * (h as i64) - desired_area).abs();
        (aspect_diff, area_diff)
    };

    candidates
        .iter()
        .copied()
        .min_by(|&a, &b| score(a).partial_cmp(&score(b)).unwrap())
}
