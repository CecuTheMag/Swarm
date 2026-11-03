//! `glyphs`: procedural handwritten-style digits, 14 × 14 (spec 0011).
//!
//! Every device generates the same images from the dataset seed, so nothing is
//! downloaded and nothing personal is ever in it. Each digit is a polyline whose
//! points are jittered, then scaled, slanted and shifted, drawn with a random stroke
//! width and speckled with noise. Only `+ − × ÷` and `sqrt` on `f32` are used: they
//! are correctly rounded in IEEE 754, so native code and WebAssembly produce the same
//! bytes (no `sin`, `exp` or other library functions, whose last bit may differ).

use super::Dataset;
use crate::rng::{sample2, unit_base};

pub const SIDE: usize = 14;
pub const DIM: usize = SIDE * SIDE;
pub const TRAIN: usize = 4096;
pub const VALIDATION: usize = 1024;
/// The one dataset seed: `glyphs` is a fixed dataset, like MNIST.
pub const SEED: u32 = 0x5157_4152;

/// How far each stroke point may move, in units of the box.
const JITTER: f32 = 0.08;
/// Pixel noise, as a share of full ink.
const NOISE: f32 = 0.22;

/// Strokes of each digit in a unit box (x right, y down).
const STROKES: [&[(f32, f32)]; 10] = [
    &[
        (0.5, 0.05),
        (0.8, 0.2),
        (0.86, 0.5),
        (0.8, 0.8),
        (0.5, 0.95),
        (0.2, 0.8),
        (0.14, 0.5),
        (0.2, 0.2),
        (0.5, 0.05),
    ],
    &[(0.32, 0.26), (0.55, 0.05), (0.55, 0.95)],
    &[
        (0.2, 0.25),
        (0.36, 0.07),
        (0.64, 0.07),
        (0.8, 0.27),
        (0.7, 0.5),
        (0.2, 0.92),
        (0.85, 0.92),
    ],
    &[
        (0.2, 0.1),
        (0.76, 0.1),
        (0.45, 0.44),
        (0.76, 0.6),
        (0.76, 0.82),
        (0.5, 0.95),
        (0.2, 0.85),
    ],
    &[(0.66, 0.95), (0.66, 0.05), (0.15, 0.65), (0.86, 0.65)],
    &[
        (0.8, 0.07),
        (0.26, 0.07),
        (0.22, 0.45),
        (0.6, 0.41),
        (0.8, 0.62),
        (0.7, 0.88),
        (0.45, 0.95),
        (0.2, 0.85),
    ],
    &[
        (0.72, 0.07),
        (0.36, 0.34),
        (0.2, 0.66),
        (0.3, 0.9),
        (0.6, 0.93),
        (0.78, 0.71),
        (0.6, 0.52),
        (0.28, 0.56),
    ],
    &[(0.14, 0.08), (0.86, 0.08), (0.44, 0.95)],
    &[
        (0.5, 0.5),
        (0.25, 0.3),
        (0.5, 0.06),
        (0.75, 0.3),
        (0.5, 0.5),
        (0.2, 0.72),
        (0.5, 0.95),
        (0.8, 0.72),
        (0.5, 0.5),
    ],
    &[
        (0.74, 0.44),
        (0.4, 0.48),
        (0.22, 0.27),
        (0.46, 0.06),
        (0.74, 0.2),
        (0.74, 0.44),
        (0.6, 0.95),
    ],
];

/// A uniform `f32` in `[lo, hi)` from 24 random bits (exactly representable).
fn uniform(bits: u32, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * ((bits >> 8) as f32 / 16_777_216.0)
}

/// Squared distance from `p` to the segment `a`–`b`.
fn segment_distance_sq(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (ex, ey) = (p.0 - (a.0 + t * dx), p.1 - (a.1 + t * dy));
    ex * ex + ey * ey
}

/// Image `index` of the stream with `seed`: its class and its pixels.
pub fn image(seed: u32, index: u32, out: &mut [u8]) -> u8 {
    let class = (index % 10) as usize;
    let base = unit_base(seed, index);
    // Random draws are numbered; each number gives two independent values.
    let mut n = 0u32;
    let mut draw = || {
        n += 1;
        sample2(base, n)
    };

    let (a, b) = draw();
    let (sx, sy) = (uniform(a, 0.6, 1.05), uniform(b, 0.7, 1.05));
    let (a, b) = draw();
    let (slant, width) = (uniform(a, -0.5, 0.5), uniform(b, 0.045, 0.13));
    let (a, b) = draw();
    let (shift_x, shift_y) = (uniform(a, -0.15, 0.15), uniform(b, -0.12, 0.12));

    let points: Vec<(f32, f32)> = STROKES[class]
        .iter()
        .map(|&(x, y)| {
            let (a, b) = draw();
            let (x, y) = (
                x + uniform(a, -JITTER, JITTER),
                y + uniform(b, -JITTER, JITTER),
            );
            let (x, y) = (0.5 + (x - 0.5) * sx, 0.5 + (y - 0.5) * sy);
            let x = x + slant * (0.5 - y);
            // The box maps to pixels 1..13, leaving a one-pixel margin.
            (1.0 + 12.0 * (x + shift_x), 1.0 + 12.0 * (y + shift_y))
        })
        .collect();

    // Half of the images get a stray stroke, as a pen slip or a mark on the paper.
    let (a, b) = draw();
    let stray = (a & 1 == 1).then(|| {
        let (c, d) = draw();
        let start = (uniform(b, 1.0, 13.0), uniform(c, 1.0, 13.0));
        let (e, _) = draw();
        (
            start,
            (
                start.0 + uniform(d, -4.0, 4.0),
                start.1 + uniform(e, -4.0, 4.0),
            ),
        )
    });

    // Stroke half-width in pixels, and a soft edge of one pixel.
    let half = 12.0 * width;
    for py in 0..SIDE {
        for px in 0..SIDE {
            let p = (px as f32 + 0.5, py as f32 + 0.5);
            let d_sq = points
                .windows(2)
                .map(|s| segment_distance_sq(p, s[0], s[1]))
                .chain(stray.map(|(s, e)| segment_distance_sq(p, s, e)))
                .fold(f32::INFINITY, f32::min);
            let ink = (1.0 - (d_sq.sqrt() - half)).clamp(0.0, 1.0);
            let (a, _) = draw();
            let noise = uniform(a, -NOISE, NOISE);
            let v = ((ink + noise).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            out[py * SIDE + px] = v;
        }
    }
    class as u8
}

/// The `glyphs` dataset: training images, then validation images from the same stream.
pub fn dataset() -> Dataset {
    let generate = |from: usize, count: usize| {
        let mut x = vec![0u8; count * DIM];
        let mut y = vec![0u8; count];
        for i in 0..count {
            y[i] = image(SEED, (from + i) as u32, &mut x[i * DIM..(i + 1) * DIM]);
        }
        (x, y)
    };
    let (train_x, train_y) = generate(0, TRAIN);
    let (val_x, val_y) = generate(TRAIN, VALIDATION);
    Dataset {
        dim: DIM,
        train_x,
        train_y,
        val_x,
        val_y,
    }
}
