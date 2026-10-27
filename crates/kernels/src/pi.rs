//! Monte Carlo estimate of π — the pipeline smoke-test template.
//!
//! Integer-only so every device returns the same count: sample coordinates are
//! 15-bit integers and a point is inside the quarter circle when
//! `x² + y² < 32768²`. The largest possible sum, `2 · 32767²`, fits in `u32`
//! (WGSL has no 64-bit integers).

use crate::rng::{pcg_hash, unit_base};

/// Coordinates are in `0..SCALE`.
pub const SCALE: u32 = 1 << 15;
const RADIUS_SQ: u32 = SCALE * SCALE;

/// Parameters of one work unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitParams {
    pub seed: u32,
    pub unit_index: u32,
    pub samples: u32,
}

/// Whether sample `i` of the unit with base `base` lands inside the quarter circle.
#[inline]
pub fn sample_inside(base: u32, i: u32) -> bool {
    let x = pcg_hash(base.wrapping_add(i.wrapping_mul(2))) >> 17;
    let y = pcg_hash(base.wrapping_add(i.wrapping_mul(2)).wrapping_add(1)) >> 17;
    x * x + y * y < RADIUS_SQ
}

/// Number of samples inside the quarter circle for one unit.
pub fn count_inside(p: UnitParams) -> u32 {
    let base = unit_base(p.seed, p.unit_index);
    (0..p.samples).filter(|&i| sample_inside(base, i)).count() as u32
}

/// π estimate from aggregated counts.
pub fn estimate(inside: u64, total: u64) -> f64 {
    4.0 * inside as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Vector {
        seed: u32,
        unit_index: u32,
        samples: u32,
        inside: u32,
    }

    /// Test vectors shared with the WGSL kernel's browser test.
    const VECTORS: &str = include_str!("../../../templates/pi/vectors.json");

    #[test]
    fn matches_shared_vectors() {
        let vectors: Vec<Vector> = serde_json::from_str(VECTORS).unwrap();
        assert!(!vectors.is_empty());
        for v in vectors {
            let p = UnitParams {
                seed: v.seed,
                unit_index: v.unit_index,
                samples: v.samples,
            };
            assert_eq!(count_inside(p), v.inside, "{p:?}");
        }
    }

    #[test]
    fn deterministic() {
        let p = UnitParams {
            seed: 7,
            unit_index: 3,
            samples: 10_000,
        };
        assert_eq!(count_inside(p), count_inside(p));
    }

    #[test]
    fn units_differ() {
        let a = UnitParams {
            seed: 7,
            unit_index: 0,
            samples: 10_000,
        };
        let b = UnitParams { unit_index: 1, ..a };
        assert_ne!(count_inside(a), count_inside(b));
    }

    #[test]
    fn converges_to_pi() {
        let (mut inside, mut total) = (0u64, 0u64);
        for unit_index in 0..16 {
            let p = UnitParams {
                seed: 42,
                unit_index,
                samples: 1 << 20,
            };
            inside += u64::from(count_inside(p));
            total += u64::from(p.samples);
        }
        // 16.7M samples: standard error ≈ 4e-4.
        assert!((estimate(inside, total) - std::f64::consts::PI).abs() < 2e-3);
    }

    #[test]
    fn no_overflow_at_extremes() {
        let max = SCALE - 1;
        assert!(
            max.checked_mul(max)
                .unwrap()
                .checked_add(max * max)
                .is_some()
        );
    }
}
