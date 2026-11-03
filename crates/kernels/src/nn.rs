//! Neural network sweep (spec 0011): a multilayer perceptron trained with integer
//! arithmetic only, so every device ends with the same weights, bit for bit.
//!
//! Number formats (Qn: the integer is the value × 2ⁿ):
//!
//! | Value | Format | Range |
//! |---|---|---|
//! | pixels, activations | Q8 | pixels 0..1, activations clamped to 0..8 |
//! | master weights and biases | Q24 | clamped to ±4 |
//! | forward weights (master, rounded) | Q12 | ±4 |
//! | probabilities, output errors, backpropagated errors | Q15 | errors clipped to ±2 |
//! | weight gradients, loss | Q16 | gradients clipped to ±1 |
//!
//! Products are shifted back to their format with an arithmetic shift (floor), the
//! same in Rust and WGSL. Integer sums are exact in any order, so a GPU may add up a
//! batch across threads however it likes. The bounds above keep every intermediate
//! inside `i32` (WGSL has no 64-bit integers); tests run with overflow checks. The
//! only non-linear function, `exp` in the softmax, is the table [`tables::EXP2`] with
//! shifts; the loss uses [`tables::LOG2`].

pub mod glyphs;
pub mod tables;

use crate::rng::{sample2, unit_base};
use tables::{EXP2, LOG2};

pub const CLASSES: usize = 10;
/// Points of the training-loss curve a unit reports.
pub const CURVE_POINTS: usize = 16;

/// Largest activation (just under 8.0 in Q8).
pub const A_MAX: i32 = (8 << 8) - 1;
/// Largest master weight or bias (4.0 in Q24).
pub const W_MAX: i32 = 4 << 24;
/// Clip of backpropagated errors (2.0 in Q15).
const D_MAX: i32 = 1 << 16;
/// Clip of mean weight gradients (just under 1.0 in Q16).
const G_MAX: i32 = (1 << 16) - 1;
/// Largest learning rate, just under 0.5 in Q16; with [`G_MAX`] the update fits `i32`.
pub const MAX_LR_Q16: u32 = (1 << 15) - 1;
pub const MAX_HIDDEN: u32 = 256;
pub const MAX_BATCH_LOG2: u32 = 7;
pub const MAX_STEPS: u32 = 5000;
/// Most multiply-adds of a unit's forward passes (weights × batch × steps): about
/// 5 s on one laptop core, so no device gets a unit that runs for minutes.
pub const MAX_WORK: u64 = 2_500_000_000;

/// Training images and the held-out validation images, `dim` bytes each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dataset {
    pub dim: usize,
    pub train_x: Vec<u8>,
    pub train_y: Vec<u8>,
    pub val_x: Vec<u8>,
    pub val_y: Vec<u8>,
}

impl Dataset {
    pub fn train_len(&self) -> usize {
        self.train_y.len()
    }
}

/// One configuration of the sweep: everything a unit trains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Units in each hidden layer.
    pub hidden: u32,
    /// Hidden layers: 1 or 2.
    pub layers: u32,
    /// Learning rate in Q16.
    pub lr_q16: u32,
    /// Batch size is `1 << batch_log2`.
    pub batch_log2: u32,
    pub steps: u32,
    pub seed: u32,
}

impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=MAX_HIDDEN).contains(&self.hidden) {
            return Err(format!("hidden must be between 1 and {MAX_HIDDEN}"));
        }
        if !(1..=2).contains(&self.layers) {
            return Err("layers must be 1 or 2".into());
        }
        if self.lr_q16 > MAX_LR_Q16 {
            return Err("learning rate must be below 0.5".into());
        }
        if self.batch_log2 > MAX_BATCH_LOG2 {
            return Err("batch must be at most 128".into());
        }
        if !(1..=MAX_STEPS).contains(&self.steps) {
            return Err(format!("steps must be between 1 and {MAX_STEPS}"));
        }
        if self.work(glyphs::DIM) > MAX_WORK {
            return Err(
                "too much work for one unit: fewer steps, a smaller batch or smaller layers".into(),
            );
        }
        Ok(())
    }

    /// Multiply-adds of the forward passes over the whole training run.
    pub fn work(&self, dim: usize) -> u64 {
        let weights: u64 = self
            .sizes(dim)
            .windows(2)
            .map(|s| (s[0] * s[1]) as u64)
            .sum();
        weights * (1u64 << self.batch_log2) * u64::from(self.steps)
    }

    /// Layer sizes from input to output.
    pub fn sizes(&self, dim: usize) -> Vec<usize> {
        let mut sizes = vec![dim];
        sizes.extend(std::iter::repeat_n(
            self.hidden as usize,
            self.layers as usize,
        ));
        sizes.push(CLASSES);
        sizes
    }
}

/// A fully connected layer: `w[i * fan_out + o]` connects input `i` to output `o`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub fan_in: usize,
    pub fan_out: usize,
    /// Master weights, Q24.
    pub w: Vec<i32>,
    /// Master biases, Q24.
    pub b: Vec<i32>,
}

/// Master weight (Q24) rounded to the forward format (Q12).
#[inline]
pub fn forward_weight(m: i32) -> i32 {
    (m + (1 << 11)) >> 12
}

/// Uniform initial weights in `±sqrt(6 / fan_in)` (He et al. for ReLU), from the
/// counter-based hash: weight `i` of layer `l` depends only on (seed, l, i).
pub fn init_layers(cfg: &Config, dim: usize) -> Vec<Layer> {
    let sizes = cfg.sizes(dim);
    sizes
        .windows(2)
        .enumerate()
        .map(|(l, s)| {
            let (fan_in, fan_out) = (s[0], s[1]);
            // sqrt(6 / fan_in) in Q24 = sqrt(6 · 2⁴⁸ / fan_in), in integers.
            let limit = ((6u64 << 48) / fan_in as u64).isqrt() as i64;
            let base = unit_base(cfg.seed, 0x1000 + l as u32);
            let w = (0..fan_in * fan_out)
                .map(|i| {
                    let (h, _) = sample2(base, i as u32);
                    (((u64::from(h) * (2 * limit + 1) as u64) >> 32) as i64 - limit) as i32
                })
                .collect();
            Layer {
                fan_in,
                fan_out,
                w,
                b: vec![0; fan_out],
            }
        })
        .collect()
}

/// Which training image fills slot `b` of the batch at `step`.
#[inline]
pub fn batch_index(seed: u32, step: u32, batch: u32, b: u32, train_len: u32) -> u32 {
    let (h, _) = sample2(unit_base(seed, 0x2000), step * batch + b);
    ((u64::from(h) * u64::from(train_len)) >> 32) as u32
}

/// Outputs of one layer for the inputs `a` (Q8); hidden layers apply ReLU.
fn forward_layer(layer: &Layer, a: &[i32], hidden: bool, out: &mut Vec<i32>) {
    out.clear();
    for o in 0..layer.fan_out {
        let mut acc = 0i32; // Q16
        for (i, &x) in a.iter().enumerate() {
            acc += (x * forward_weight(layer.w[i * layer.fan_out + o])) >> 4;
        }
        let z = (acc >> 8) + (layer.b[o] >> 16);
        out.push(if hidden { z.clamp(0, A_MAX) } else { z });
    }
}

/// Activations of every layer for one image: `acts[0]` is the input, the last the logits.
fn forward(layers: &[Layer], pixels: &[u8], acts: &mut [Vec<i32>]) {
    acts[0].clear();
    acts[0].extend(pixels.iter().map(|&p| i32::from(p)));
    for (l, layer) in layers.iter().enumerate() {
        let (before, after) = acts.split_at_mut(l + 1);
        forward_layer(layer, &before[l], l + 1 < layers.len(), &mut after[0]);
    }
}

/// Softmax of logits (Q8) as probabilities (Q15) that sum to about 2¹⁵.
pub fn softmax(logits: &[i32], p: &mut [i32]) {
    let max = logits.iter().copied().max().unwrap_or(0);
    let mut e = [0u32; CLASSES];
    let mut sum = 0u32;
    for (k, &z) in logits.iter().enumerate() {
        // 2^(d · log2 e) with d ≤ 0 in Q8; 23637 / 2¹⁴ ≈ log2 e.
        let d = (z - max).max(-(32 << 8));
        let u = (d * 23637) >> 14;
        let (q, f) = (u >> 8, (u & 255) as usize);
        e[k] = if -q > 31 { 0 } else { EXP2[f] >> (-q) as u32 };
        sum += e[k];
    }
    for k in 0..logits.len() {
        p[k] = ((e[k] << 15) / sum) as i32;
    }
}

/// Cross-entropy `−ln p` in Q16 for a probability in Q15.
pub fn cross_entropy(p: i32) -> u32 {
    let x = p.clamp(1, 1 << 15) as u32;
    let k = 31 - x.leading_zeros();
    let log2_x = (k << 16) + LOG2[(((x << 8) >> k) & 255) as usize];
    // (15 − log2 x) · ln 2, with ln 2 = 45426 / 2¹⁶.
    ((((15 << 16) - log2_x) >> 4) * 45426) >> 12
}

/// Index of the largest logit (the first one on a tie).
fn argmax(v: &[i32]) -> usize {
    let mut best = 0;
    for (k, &x) in v.iter().enumerate() {
        if x > v[best] {
            best = k;
        }
    }
    best
}

/// What a unit returns, before the digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trained {
    pub layers: Vec<Layer>,
    /// Validation images classified correctly.
    pub correct: u32,
    /// Mean training loss (Q16) over each sixteenth of the steps.
    pub curve: [u32; CURVE_POINTS],
}

impl Trained {
    /// The final weights in the order the digest covers: per layer, weights then
    /// biases, little-endian.
    pub fn weight_bytes(&self) -> Vec<u8> {
        self.layers
            .iter()
            .flat_map(|l| l.w.iter().chain(&l.b))
            .flat_map(|v| v.to_le_bytes())
            .collect()
    }
}

/// Validation images that `layers` classifies correctly.
pub fn evaluate(layers: &[Layer], data: &Dataset) -> u32 {
    let mut acts = vec![Vec::new(); layers.len() + 1];
    let mut correct = 0;
    for (i, &y) in data.val_y.iter().enumerate() {
        forward(
            layers,
            &data.val_x[i * data.dim..(i + 1) * data.dim],
            &mut acts,
        );
        if argmax(&acts[layers.len()]) == usize::from(y) {
            correct += 1;
        }
    }
    correct
}

/// Trains one configuration from scratch: mini-batch SGD on the cross-entropy loss.
// Index loops mirror the WGSL kernel line by line, which keeps the two easy to compare.
#[allow(clippy::needless_range_loop)]
pub fn train(cfg: &Config, data: &Dataset) -> Trained {
    let mut layers = init_layers(cfg, data.dim);
    let n = layers.len();
    let batch = 1u32 << cfg.batch_log2;
    let train_len = data.train_len() as u32;
    let lr = cfg.lr_q16 as i32;

    let mut acts = vec![Vec::new(); n + 1];
    let mut p = [0i32; CLASSES];
    let mut delta: Vec<Vec<i32>> = layers.iter().map(|l| vec![0; l.fan_out]).collect();
    let mut grad_w: Vec<Vec<i32>> = layers.iter().map(|l| vec![0; l.w.len()]).collect();
    let mut grad_b: Vec<Vec<i32>> = layers.iter().map(|l| vec![0; l.fan_out]).collect();

    let mut curve_sum = [0u32; CURVE_POINTS];
    let mut curve_steps = [0u32; CURVE_POINTS];

    for step in 0..cfg.steps {
        grad_w.iter_mut().for_each(|g| g.fill(0));
        grad_b.iter_mut().for_each(|g| g.fill(0));
        let mut loss = 0u32; // Q16, summed over the batch

        for b in 0..batch {
            let i = batch_index(cfg.seed, step, batch, b, train_len) as usize;
            forward(
                &layers,
                &data.train_x[i * data.dim..(i + 1) * data.dim],
                &mut acts,
            );
            let y = usize::from(data.train_y[i]);
            softmax(&acts[n], &mut p);
            loss += cross_entropy(p[y]);

            // Output error p − y (Q15), then backwards through the hidden layers.
            for k in 0..CLASSES {
                delta[n - 1][k] = p[k] - if k == y { 1 << 15 } else { 0 };
            }
            for l in (0..n).rev() {
                let layer = &layers[l];
                let input = &acts[l];
                for (i, &a) in input.iter().enumerate() {
                    for o in 0..layer.fan_out {
                        grad_w[l][i * layer.fan_out + o] += (delta[l][o] * a) >> 7;
                    }
                }
                for o in 0..layer.fan_out {
                    grad_b[l][o] += delta[l][o] << 1;
                }
                if l > 0 {
                    let (before, after) = delta.split_at_mut(l);
                    for (i, &a) in input.iter().enumerate() {
                        let mut d = 0i32;
                        for o in 0..layer.fan_out {
                            d += (after[0][o] * forward_weight(layer.w[i * layer.fan_out + o]))
                                >> 12;
                        }
                        // ReLU and the clamp at A_MAX pass no gradient.
                        before[l - 1][i] = if a > 0 && a < A_MAX {
                            d.clamp(-D_MAX, D_MAX)
                        } else {
                            0
                        };
                    }
                }
            }
        }

        // Mean gradient over the batch, clipped, then the SGD update with rounding.
        for (l, layer) in layers.iter_mut().enumerate() {
            let update = |m: &mut i32, g: i32| {
                let g = (g >> cfg.batch_log2).clamp(-G_MAX, G_MAX);
                *m = (*m - ((lr * g + 128) >> 8)).clamp(-W_MAX, W_MAX);
            };
            for (m, &g) in layer.w.iter_mut().zip(&grad_w[l]) {
                update(m, g);
            }
            for (m, &g) in layer.b.iter_mut().zip(&grad_b[l]) {
                update(m, g);
            }
        }

        let point = (step as usize * CURVE_POINTS) / cfg.steps as usize;
        curve_sum[point] += loss >> cfg.batch_log2;
        curve_steps[point] += 1;
    }

    let mut curve = [0u32; CURVE_POINTS];
    for k in 0..CURVE_POINTS {
        curve[k] = curve_sum[k].checked_div(curve_steps[k]).unwrap_or(0);
    }
    let correct = evaluate(&layers, data);
    Trained {
        layers,
        correct,
        curve,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(hidden: u32, layers: u32, lr: f64, batch_log2: u32, steps: u32) -> Config {
        Config {
            hidden,
            layers,
            lr_q16: (lr * 65536.0).round() as u32,
            batch_log2,
            steps,
            seed: 7,
        }
    }

    #[test]
    fn tables_match_their_definition() {
        for f in 0..256 {
            let exp2 = (32768.0 * 2f64.powf(f as f64 / 256.0)).round() as u32;
            assert_eq!(EXP2[f], exp2, "EXP2[{f}]");
            let log2 = (65536.0 * (1.0 + f as f64 / 256.0).log2()).round() as u32;
            assert_eq!(LOG2[f], log2, "LOG2[{f}]");
        }
    }

    #[test]
    fn softmax_and_loss_are_close_to_floating_point() {
        let logits = [0, 256, -512, 1024, 40, -3000, 700, 0, 5, 2047];
        let mut p = [0; CLASSES];
        softmax(&logits, &mut p);
        let exact: Vec<f64> = {
            let e: Vec<f64> = logits.iter().map(|&z| (z as f64 / 256.0).exp()).collect();
            let s: f64 = e.iter().sum();
            e.iter().map(|v| v / s).collect()
        };
        for k in 0..CLASSES {
            assert!((p[k] as f64 / 32768.0 - exact[k]).abs() < 2e-3, "p[{k}]");
        }
        for (q15, want) in [
            (1 << 15, 0.0),
            (1 << 14, 2f64.ln()),
            (100, (32768f64 / 100.0).ln()),
        ] {
            let got = cross_entropy(q15) as f64 / 65536.0;
            assert!((got - want).abs() < 2e-3, "loss of {q15}: {got} vs {want}");
        }
        // A probability of zero costs the most, not an overflow.
        assert!(cross_entropy(0) > cross_entropy(1 << 10));
    }

    #[test]
    fn glyphs_are_deterministic_and_balanced() {
        let a = glyphs::dataset();
        assert_eq!(a, glyphs::dataset());
        assert_eq!(a.train_x.len(), glyphs::TRAIN * glyphs::DIM);
        for class in 0..10u8 {
            let n = a.train_y.iter().filter(|&&y| y == class).count();
            assert!(n >= glyphs::TRAIN / 10 - 1, "class {class}: {n}");
        }
        // Images have ink and background, not blank or saturated.
        let ink = a.train_x.iter().filter(|&&v| v > 128).count() as f64 / a.train_x.len() as f64;
        assert!((0.08..0.5).contains(&ink), "ink {ink}");
    }

    #[test]
    fn training_learns_and_is_deterministic() {
        let data = glyphs::dataset();
        let c = cfg(64, 1, 0.2, 5, 400);
        let a = train(&c, &data);
        assert_eq!(a, train(&c, &data), "same configuration, same bits");
        // Chance is 10 %; examples/nn_sweep.rs shows 55–85 % across settings.
        let accuracy = a.correct as f64 / data.val_y.len() as f64;
        assert!(accuracy > 0.6, "accuracy {accuracy}");
        assert!(a.curve[CURVE_POINTS - 1] < a.curve[0] / 2, "{:?}", a.curve);
        // Another seed starts elsewhere and ends elsewhere.
        assert_ne!(train(&Config { seed: 8, ..c }, &data).layers, a.layers);
    }

    #[test]
    fn a_learning_rate_of_zero_learns_nothing() {
        let data = glyphs::dataset();
        let c = cfg(32, 1, 0.0, 4, 50);
        let t = train(&c, &data);
        assert_eq!(t.layers, init_layers(&c, data.dim));
        assert!(t.correct < data.val_y.len() as u32 / 4);
    }

    #[test]
    fn work_is_capped() {
        assert!(cfg(128, 1, 0.1, 5, 600).validate().is_ok());
        assert!(cfg(256, 2, 0.1, 7, 5000).validate().is_err());
        assert!(cfg(0, 1, 0.1, 5, 600).validate().is_err());
        assert!(cfg(64, 3, 0.1, 5, 600).validate().is_err());
        assert!(cfg(64, 1, 0.5, 5, 600).validate().is_err());
    }

    #[test]
    fn extreme_settings_stay_within_i32() {
        // Debug builds check every operation for overflow.
        let data = glyphs::dataset();
        for c in [
            cfg(MAX_HIDDEN, 2, 0.49, MAX_BATCH_LOG2, 30),
            cfg(1, 1, 0.49, 0, 200),
            cfg(256, 1, 0.3, 3, 60),
        ] {
            train(&c, &data);
        }
    }
}
