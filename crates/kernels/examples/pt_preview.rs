//! Renders the path tracer's scene on this machine and writes a PPM image to stdout.
//!
//!     cargo run --release -p kernels --example pt_preview -- 320 240 64 > preview.ppm
//!
//! Arguments: width, height, samples per pixel (defaults 320, 240, 64).

use kernels::pt::{TileParams, render_tile};
use std::io::Write;

fn main() -> std::io::Result<()> {
    let args: Vec<u32> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("arguments are numbers"))
        .collect();
    let (width, height, spp) = (
        args.first().copied().unwrap_or(320),
        args.get(1).copied().unwrap_or(240),
        args.get(2).copied().unwrap_or(64),
    );
    let p = TileParams {
        width,
        height,
        x: 0,
        y: 0,
        w: width,
        h: height,
        spp,
        pass: 0,
        seed: 1,
    };
    let mut sums = vec![0.0; (width * height * 3) as usize];
    render_tile(&p, &mut sums);

    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    write!(out, "P6\n{width} {height}\n255\n")?;
    let bytes: Vec<u8> = sums
        .iter()
        .map(|s| ((s / spp as f32).clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8)
        .collect();
    out.write_all(&bytes)
}
