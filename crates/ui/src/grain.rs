use std::sync::{Arc, OnceLock};

use gpui::prelude::*;
use gpui::{Div, RenderImage, Window, div, img, px};
use image::{Frame, RgbaImage};

/// How wide the tile is, in device pixels. Wider means fewer quads to lay it out with and a
/// repeat nobody can see anyway at this strength.
const TILE: u32 = 512;
/// How far a pixel of the tile can push what it covers, out of 255. A gradient wide enough to
/// band steps by one level at a time, so a couple of levels is the whole budget: one to
/// scatter the step, the rest is the grain itself.
const PUSH: u8 = 2;

/// The dither tile, built once for the process. Each pixel is nudged toward white or toward
/// black by up to `PUSH`, which spreads every eight bit step of a gradient across a band of
/// pixels instead of leaving it a contour that holds still and then jumps a whole band as the
/// gradient drifts.
fn tile() -> Arc<RenderImage> {
    static TILED: OnceLock<Arc<RenderImage>> = OnceLock::new();

    TILED
        .get_or_init(|| {
            let mut seed = 0x2545_f491_4f6c_dd1d_u64;
            let mut roll = move || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed
            };
            let mut pixels = RgbaImage::new(TILE, TILE);
            for pixel in pixels.pixels_mut() {
                let draw = roll();
                let level = match draw & 1 {
                    0 => 0,
                    _ => 255,
                };
                let alpha = (draw >> 8) as u8 % (PUSH + 1);
                *pixel = image::Rgba([level, level, level, alpha]);
            }
            Arc::new(RenderImage::new(vec![Frame::new(pixels)]))
        })
        .clone()
}

/// Lays dither over whatever it covers, for a wide gradient that would otherwise band. The
/// parent has to be `relative`, and the tile is sized in device pixels so the grain stays one
/// pixel wide however the display is scaled.
pub fn grain(window: &Window) -> Div {
    let scale = window.scale_factor().max(1.);
    let side = TILE as f32 / scale;
    let viewport = window.viewport_size();
    let across = (viewport.width.as_f32() / side).ceil().max(1.) as usize;
    let down = (viewport.height.as_f32() / side).ceil().max(1.) as usize;

    // Placed by hand rather than wrapped: a wrap breaks on the container's width, so the
    // partial column the window is left with never gets a tile and the edge stays bare.
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .children((0..down).flat_map(move |row| {
            (0..across).map(move |column| {
                img(tile())
                    .absolute()
                    .left(px(column as f32 * side))
                    .top(px(row as f32 * side))
                    .w(px(side))
                    .h(px(side))
            })
        }))
}
