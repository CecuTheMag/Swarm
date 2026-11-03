//! Accuracy and speed of the integer trainer on `glyphs`, next to the same network
//! trained in `f32` (same initial weights, same batches): what integers cost.
//!
//! `cargo run --release -p kernels --example nn_sweep`

use kernels::nn::{self, Config, Dataset, glyphs};
use std::time::Instant;

/// The same network in floating point, as the yardstick.
fn train_f32(cfg: &Config, data: &Dataset) -> u32 {
    let init = nn::init_layers(cfg, data.dim);
    let mut w: Vec<Vec<f32>> = init
        .iter()
        .map(|l| l.w.iter().map(|&v| v as f32 / 16_777_216.0).collect())
        .collect();
    let mut b: Vec<Vec<f32>> = init.iter().map(|l| vec![0.0; l.fan_out]).collect();
    let sizes = cfg.sizes(data.dim);
    let n = sizes.len() - 1;
    let batch = 1u32 << cfg.batch_log2;
    let lr = cfg.lr_q16 as f32 / 65536.0;
    let fwd = |w: &[Vec<f32>], b: &[Vec<f32>], x: &[u8]| {
        let mut acts = vec![x.iter().map(|&p| p as f32 / 256.0).collect::<Vec<f32>>()];
        for l in 0..n {
            let out: Vec<f32> = (0..sizes[l + 1])
                .map(|o| {
                    let z = b[l][o]
                        + acts[l]
                            .iter()
                            .enumerate()
                            .map(|(i, a)| a * w[l][i * sizes[l + 1] + o])
                            .sum::<f32>();
                    if l + 1 < n { z.max(0.0) } else { z }
                })
                .collect();
            acts.push(out);
        }
        acts
    };
    for step in 0..cfg.steps {
        let mut gw: Vec<Vec<f32>> = w.iter().map(|v| vec![0.0; v.len()]).collect();
        let mut gb: Vec<Vec<f32>> = b.iter().map(|v| vec![0.0; v.len()]).collect();
        for s in 0..batch {
            let i = nn::batch_index(cfg.seed, step, batch, s, data.train_len() as u32) as usize;
            let acts = fwd(&w, &b, &data.train_x[i * data.dim..(i + 1) * data.dim]);
            let z = &acts[n];
            let m = z.iter().cloned().fold(f32::MIN, f32::max);
            let e: Vec<f32> = z.iter().map(|v| (v - m).exp()).collect();
            let sum: f32 = e.iter().sum();
            let mut d: Vec<f32> = e
                .iter()
                .enumerate()
                .map(|(k, v)| {
                    v / sum
                        - if k == data.train_y[i] as usize {
                            1.0
                        } else {
                            0.0
                        }
                })
                .collect();
            for l in (0..n).rev() {
                for (ii, a) in acts[l].iter().enumerate() {
                    for o in 0..sizes[l + 1] {
                        gw[l][ii * sizes[l + 1] + o] += d[o] * a;
                    }
                }
                for o in 0..sizes[l + 1] {
                    gb[l][o] += d[o];
                }
                if l > 0 {
                    d = (0..sizes[l])
                        .map(|ii| {
                            let s: f32 = (0..sizes[l + 1])
                                .map(|o| d[o] * w[l][ii * sizes[l + 1] + o])
                                .sum();
                            if acts[l][ii] > 0.0 { s } else { 0.0 }
                        })
                        .collect();
                }
            }
        }
        for l in 0..n {
            for (v, g) in w[l].iter_mut().zip(&gw[l]) {
                *v -= lr * g / batch as f32;
            }
            for (v, g) in b[l].iter_mut().zip(&gb[l]) {
                *v -= lr * g / batch as f32;
            }
        }
    }
    (0..data.val_y.len())
        .filter(|&i| {
            let acts = fwd(&w, &b, &data.val_x[i * data.dim..(i + 1) * data.dim]);
            let z = &acts[n];
            let best = (0..z.len()).fold(0, |bst, k| if z[k] > z[bst] { k } else { bst });
            best == data.val_y[i] as usize
        })
        .count() as u32
}

fn main() {
    let data = glyphs::dataset();
    let val = data.val_y.len() as f64;
    println!("| hidden | layers | lr | batch | steps | integer | f32 | ms (integer) |");
    println!("|---|---|---|---|---|---|---|---|");
    for (hidden, layers, lr, batch_log2, steps) in [
        (32, 1, 0.05, 5, 600),
        (32, 1, 0.2, 5, 600),
        (64, 1, 0.1, 5, 600),
        (128, 1, 0.1, 5, 600),
        (128, 1, 0.3, 5, 600),
        (64, 2, 0.1, 5, 600),
        (128, 2, 0.1, 5, 1000),
        (128, 1, 0.02, 5, 600),
        (16, 1, 0.1, 4, 600),
        (256, 1, 0.1, 6, 600),
    ] {
        let cfg = Config {
            hidden,
            layers,
            lr_q16: (lr * 65536.0f64).round() as u32,
            batch_log2,
            steps,
            seed: 1,
        };
        let t = Instant::now();
        let trained = nn::train(&cfg, &data);
        let ms = t.elapsed().as_millis();
        let f = train_f32(&cfg, &data);
        println!(
            "| {hidden} | {layers} | {lr} | {} | {steps} | {:.1} % | {:.1} % | {ms} |",
            1 << batch_log2,
            100.0 * trained.correct as f64 / val,
            100.0 * f as f64 / val
        );
    }
}
