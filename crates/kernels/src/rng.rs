//! Stateless, counter-based random numbers.
//!
//! Every random value is a pure function of its coordinates (seed, unit, sample),
//! so results do not depend on thread scheduling and are bit-identical on every
//! device. The WGSL kernels implement exactly the same functions.
//!
//! Choice validated empirically (see docs/measurements.md): hashing consecutive
//! integers with `pcg_hash` biased the π estimate by ~8 standard errors;
//! `pcg2d` is unbiased but its samples are slightly correlated (variance 30%
//! below binomial); `pcg2d` followed by `lowbias32` is unbiased with binomial
//! variance, i.e. behaves like independent uniform samples.

/// PCG-based integer hash (Jarzynski & Olano, "Hash Functions for GPU Rendering", 2020).
#[inline]
pub fn pcg_hash(input: u32) -> u32 {
    let state = input.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

/// Two-dimensional PCG hash (Jarzynski & Olano, 2020).
#[inline]
pub fn pcg2d(a: u32, b: u32) -> (u32, u32) {
    const M: u32 = 1_664_525;
    const C: u32 = 1_013_904_223;
    let mut x = a.wrapping_mul(M).wrapping_add(C);
    let mut y = b.wrapping_mul(M).wrapping_add(C);
    x = x.wrapping_add(y.wrapping_mul(M));
    y = y.wrapping_add(x.wrapping_mul(M));
    x ^= x >> 16;
    y ^= y >> 16;
    x = x.wrapping_add(y.wrapping_mul(M));
    y = y.wrapping_add(x.wrapping_mul(M));
    x ^= x >> 16;
    y ^= y >> 16;
    (x, y)
}

/// Integer finaliser with low bias (Chris Wellons, "Prospecting for Hash Functions").
#[inline]
pub fn lowbias32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// Base value for one work unit; per-sample values are derived from it.
#[inline]
pub fn unit_base(seed: u32, unit_index: u32) -> u32 {
    pcg_hash(seed ^ pcg_hash(unit_index))
}

/// Two independent uniform `u32` values for sample `i` of the unit with `base`.
#[inline]
pub fn sample2(base: u32, i: u32) -> (u32, u32) {
    let (x, y) = pcg2d(i, base);
    (lowbias32(x), lowbias32(y))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pinned values (computed by an independent implementation) so any
    // accidental change is caught; the WGSL port is tested against the same
    // functions through the shared π vectors.
    #[test]
    fn pcg_hash_known_values() {
        assert_eq!(pcg_hash(0), 129_708_002);
        assert_eq!(pcg_hash(1), 2_831_084_092);
        assert_eq!(pcg_hash(u32::MAX), 3_861_530_882);
    }

    #[test]
    fn sample2_known_values() {
        assert_eq!(sample2(0, 0), (4_267_951_872, 3_397_549_720));
        assert_eq!(sample2(123_456_789, 42), (708_397_654, 3_434_845_102));
    }
}
