//! Monte Carlo estimate of π — the pipeline smoke-test template.
//!
//! Integer-only so every device returns the same count. Each sample picks one
//! cell of a 16384 × 16384 grid over the unit square and tests the cell's
//! centre: with doubled coordinates `X = 2x + 1` (odd, `1..=32767`) the point is
//! inside the quarter circle when `X² + Y² < 32768²`. Testing centres instead of
//! corners makes the grid's own bias negligible (~1e-6). The largest possible
//! sum, `2 · 32767²`, fits in `u32` (WGSL has no 64-bit integers).

use crate::rng::{sample2, unit_base};

/// Grid cells per axis.
pub const GRID: u32 = 1 << 14;
const RADIUS_SQ: u32 = (2 * GRID) * (2 * GRID);

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
    let (a, b) = sample2(base, i);
    let x = 2 * (a >> 18) + 1;
    let y = 2 * (b >> 18) + 1;
    x * x + y * y < RADIUS_SQ
}

/// Number of samples inside the quarter circle for one unit.
pub fn count_inside(p: UnitParams) -> u32 {
    count_inside_range(p.seed, p.unit_index, 0, p.samples)
}

/// Number of samples `start..end` of a unit inside the quarter circle. Counts of
/// disjoint ranges add up to the unit's count, so a unit can be split across threads.
pub fn count_inside_range(seed: u32, unit_index: u32, start: u32, end: u32) -> u32 {
    let base = unit_base(seed, unit_index);
    (start..end).filter(|&i| sample_inside(base, i)).count() as u32
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
    fn ranges_add_up_to_the_unit() {
        let p = UnitParams {
            seed: 42,
            unit_index: 63,
            samples: 100_003,
        };
        let split = [0, 1, 4096, 50_000, 99_999, 100_003];
        let sum: u32 = split
            .windows(2)
            .map(|w| count_inside_range(p.seed, p.unit_index, w[0], w[1]))
            .sum();
        assert_eq!(sum, count_inside(p));
        assert_eq!(count_inside_range(p.seed, p.unit_index, 7, 7), 0);
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
        let max = 2 * (GRID - 1) + 1;
        assert!(
            max.checked_mul(max)
                .unwrap()
                .checked_add(max * max)
                .is_some()
        );
    }
}
