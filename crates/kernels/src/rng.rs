//! Stateless, counter-based random numbers.
//!
//! Every random value is a pure function of its coordinates (seed, unit, sample),
//! so results do not depend on thread scheduling and are bit-identical on every
//! device. The WGSL kernels implement exactly the same functions.

/// PCG-based integer hash (Jarzynski & Olano, "Hash Functions for GPU Rendering", 2020).
#[inline]
pub fn pcg_hash(input: u32) -> u32 {
    let state = input.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

/// Base value for one work unit; per-sample values are derived from it.
#[inline]
pub fn unit_base(seed: u32, unit_index: u32) -> u32 {
    pcg_hash(seed ^ pcg_hash(unit_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcg_hash_known_values() {
        // Pinned so any accidental change to the hash is caught; the WGSL port
        // is tested against the same values.
        assert_eq!(pcg_hash(0), 129_708_002);
        assert_eq!(pcg_hash(1), 2_831_084_092);
        assert_eq!(pcg_hash(u32::MAX), 3_861_530_882);
    }
}
