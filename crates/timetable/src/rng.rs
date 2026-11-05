//! SplitMix64: a small generator whose output depends only on its seed, the same on
//! every platform. Integer only, so a search gives the same timetable natively and in
//! WebAssembly.

#[derive(Debug, Clone)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`, by multiplying the top 32 bits (Lemire's method without the
    /// rejection step: a bias below n / 2³², irrelevant to a search).
    pub(crate) fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0 && n <= u32::MAX as usize);
        (((self.next_u64() >> 32) * n as u64) >> 32) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        // SplitMix64 from seed 0, as published with the algorithm.
        let mut r = Rng::new(0);
        assert_eq!(r.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(r.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    }

    #[test]
    fn below_covers_its_range_evenly() {
        let mut r = Rng::new(7);
        let mut seen = [0u32; 10];
        for _ in 0..100_000 {
            seen[r.below(10)] += 1;
        }
        assert!(
            seen.iter().all(|&n| (9_500..10_500).contains(&n)),
            "{seen:?}"
        );
    }
}
