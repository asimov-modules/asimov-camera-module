// This is free and unencumbered software released into the public domain.

use std::sync::Arc;

use crate::{Frame, FrameRef, PixelFormat, RawFormat, RawFrame, RawFrameRef};

#[inline]
fn clamp_u8(v: i32) -> u8 {
    if v <= 0 {
        0
    } else if v >= 255 {
        255
    } else {
        v as u8
    }
}

#[inline]
fn checked_min_row_bytes(width: u32, bytes_per_pixel: u32) -> usize {
    (width as usize).saturating_mul(bytes_per_pixel as usize)
}

#[inline]
fn get_packed_plane(raw: &RawFrame) -> Option<(&[u8], u32, u32)> {
    let p = raw.planes.get(0)?;
    Some((p.data.as_ref(), p.row_stride, p.pixel_stride))
}

pub fn convert_to_packed_rgb8(raw_ref: RawFrameRef) -> Option<RawFrameRef> {
    let raw = raw_ref.as_ref();

    match raw.format {
        RawFormat::PackedRgb8 => Some(raw_ref),

        RawFormat::PackedBgra8 => {
            let (src, row_stride, pixel_stride) = get_packed_plane(raw)?;
            let rgb = bgra_to_rgb(src, raw.width, raw.height, row_stride, pixel_stride)?;
            let out = RawFrame::new_rgb8(
                raw.width,
                raw.height,
                rgb,
                raw.width.saturating_mul(3),
                raw.timestamp_ns,
            );
            Some(Arc::new(out))
        },

        RawFormat::AndroidYuv420888 => {
            let rgb = yuv420888_to_rgb8(raw_ref.clone())?;
            let out = RawFrame::new_rgb8(
                raw.width,
                raw.height,
                rgb,
                raw.width.saturating_mul(3),
                raw.timestamp_ns,
            );
            Some(Arc::new(out))
        },
    }
}

pub fn convert_to_packed_bgra8(raw_ref: RawFrameRef) -> Option<RawFrameRef> {
    let raw = raw_ref.as_ref();

    match raw.format {
        RawFormat::PackedBgra8 => Some(raw_ref),

        RawFormat::PackedRgb8 => {
            let (src, row_stride, pixel_stride) = get_packed_plane(raw)?;
            let bgra = rgb_to_bgra(src, raw.width, raw.height, row_stride, pixel_stride)?;
            let out = RawFrame::new_bgra8(
                raw.width,
                raw.height,
                bgra,
                raw.width.saturating_mul(4),
                raw.timestamp_ns,
            );
            Some(Arc::new(out))
        },

        RawFormat::AndroidYuv420888 => {
            let rgb_ref = convert_to_packed_rgb8(raw_ref)?;
            let rgb = rgb_ref.as_ref();
            let (src, row_stride, pixel_stride) = get_packed_plane(rgb)?;
            let bgra = rgb_to_bgra(src, rgb.width, rgb.height, row_stride, pixel_stride)?;
            let out = RawFrame::new_bgra8(
                rgb.width,
                rgb.height,
                bgra,
                rgb.width.saturating_mul(4),
                rgb.timestamp_ns,
            );
            Some(Arc::new(out))
        },
    }
}

pub fn convert_raw_to_frame(raw_ref: RawFrameRef, output: PixelFormat) -> Option<FrameRef> {
    match output {
        PixelFormat::Rgb8 => {
            let packed_ref = convert_to_packed_rgb8(raw_ref)?;
            let raw = packed_ref.as_ref();

            if raw.format != RawFormat::PackedRgb8 || raw.planes.is_empty() {
                return None;
            }
            let p0 = raw.planes.get(0)?;
            let min_row = checked_min_row_bytes(raw.width, 3);
            if (p0.row_stride as usize) < min_row || p0.pixel_stride != 3 {
                return None;
            }

            Some(Arc::new(Frame::new(
                raw.width,
                raw.height,
                p0.row_stride.max(raw.width.saturating_mul(3)),
                PixelFormat::Rgb8,
                p0.data.clone(),
                raw.timestamp_ns,
            )))
        },

        PixelFormat::Bgra8 => {
            let packed_ref = convert_to_packed_bgra8(raw_ref)?;
            let raw = packed_ref.as_ref();

            if raw.format != RawFormat::PackedBgra8 || raw.planes.is_empty() {
                return None;
            }
            let p0 = raw.planes.get(0)?;
            let min_row = checked_min_row_bytes(raw.width, 4);
            if (p0.row_stride as usize) < min_row || p0.pixel_stride != 4 {
                return None;
            }

            Some(Arc::new(Frame {
                width: raw.width,
                height: raw.height,
                stride: p0.row_stride.max(raw.width.saturating_mul(4)),
                pixel_format: PixelFormat::Bgra8,
                data: p0.data.clone(),
                timestamp_ns: raw.timestamp_ns,
            }))
        },

        PixelFormat::I420 => None,
    }
}

fn bgra_to_rgb(src: &[u8], w: u32, h: u32, row_stride: u32, pixel_stride: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || pixel_stride == 0 || row_stride == 0 {
        return None;
    }

    let w_us = w as usize;
    let h_us = h as usize;
    let rs = row_stride as usize;
    let ps = pixel_stride as usize;

    if ps < 4 {
        return None;
    }

    let mut out = vec![0u8; w_us.saturating_mul(h_us).saturating_mul(3)];

    for y in 0..h_us {
        let row_off = y.checked_mul(rs)?;
        for x in 0..w_us {
            let i = row_off.checked_add(x.checked_mul(ps)?)?;
            if i + 3 >= src.len() {
                return None;
            }

            let b = src[i + 0];
            let g = src[i + 1];
            let r = src[i + 2];

            let o = (y * w_us + x) * 3;
            out[o + 0] = r;
            out[o + 1] = g;
            out[o + 2] = b;
        }
    }

    Some(out)
}

fn rgb_to_bgra(src: &[u8], w: u32, h: u32, row_stride: u32, pixel_stride: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || pixel_stride == 0 || row_stride == 0 {
        return None;
    }

    let w_us = w as usize;
    let h_us = h as usize;
    let rs = row_stride as usize;
    let ps = pixel_stride as usize;

    if ps < 3 {
        return None;
    }

    let mut out = vec![0u8; w_us.saturating_mul(h_us).saturating_mul(4)];

    for y in 0..h_us {
        let row_off = y.checked_mul(rs)?;
        for x in 0..w_us {
            let i = row_off.checked_add(x.checked_mul(ps)?)?;
            if i + 2 >= src.len() {
                return None;
            }

            let r = src[i + 0];
            let g = src[i + 1];
            let b = src[i + 2];

            let o = (y * w_us + x) * 4;
            out[o + 0] = b;
            out[o + 1] = g;
            out[o + 2] = r;
            out[o + 3] = 255;
        }
    }

    Some(out)
}

fn yuv420888_to_rgb8(raw_ref: RawFrameRef) -> Option<Vec<u8>> {
    let raw = raw_ref.as_ref();

    let w = raw.width as usize;
    let h = raw.height as usize;
    if w == 0 || h == 0 {
        return None;
    }
    if raw.planes.len() < 3 {
        return None;
    }

    let y_plane = &raw.planes[0];
    let u_plane = &raw.planes[1];
    let v_plane = &raw.planes[2];

    let y = y_plane.data.as_ref();
    let u = u_plane.data.as_ref();
    let v = v_plane.data.as_ref();

    let y_rs = y_plane.row_stride as usize;
    let y_ps = y_plane.pixel_stride as usize;

    let u_rs = u_plane.row_stride as usize;
    let u_ps = u_plane.pixel_stride as usize;

    let v_rs = v_plane.row_stride as usize;
    let v_ps = v_plane.pixel_stride as usize;

    if y_rs == 0 || y_ps == 0 || u_rs == 0 || u_ps == 0 || v_rs == 0 || v_ps == 0 {
        return None;
    }

    let y_min_last = (h - 1)
        .checked_mul(y_rs)?
        .checked_add((w - 1).checked_mul(y_ps)?)?;
    if y_min_last >= y.len() {
        return None;
    }

    let mut out = vec![0u8; w.saturating_mul(h).saturating_mul(3)];

    for yy in 0..h {
        let y_row = yy.checked_mul(y_rs)?;
        let u_row = (yy / 2).checked_mul(u_rs)?;
        let v_row = (yy / 2).checked_mul(v_rs)?;

        for xx in 0..w {
            let y_idx = y_row.checked_add(xx.checked_mul(y_ps)?)?;
            let u_idx = u_row.checked_add((xx / 2).checked_mul(u_ps)?)?;
            let v_idx = v_row.checked_add((xx / 2).checked_mul(v_ps)?)?;

            if y_idx >= y.len() || u_idx >= u.len() || v_idx >= v.len() {
                return None;
            }

            let yv = y[y_idx] as i32;
            let uv = u[u_idx] as i32;
            let vv = v[v_idx] as i32;

            let c = yv - 16;
            let d = uv - 128;
            let e = vv - 128;

            let r = (298 * c + 409 * e + 128) >> 8;
            let g = (298 * c - 100 * d - 208 * e + 128) >> 8;
            let b = (298 * c + 516 * d + 128) >> 8;

            let o = (yy * w + xx) * 3;
            out[o + 0] = clamp_u8(r);
            out[o + 1] = clamp_u8(g);
            out[o + 2] = clamp_u8(b);
        }
    }

    Some(out)
}
