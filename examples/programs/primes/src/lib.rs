//! Counts the primes below a bound, split into blocks: one block per unit.
//!
//! Submit it with `count` units and `args` = the block size as 8 little-endian bytes.
//! Unit `i` receives `i` (4 bytes) then the block size, sieves `[i·block, (i+1)·block)`
//! and returns the number of primes in it as 8 little-endian bytes. The units' counts
//! add up to π(count · block), which is how the example checks itself.

/// Primes in `[start, end)`, by a segmented sieve of Eratosthenes.
pub fn primes_in(start: u64, end: u64) -> u64 {
    if end <= 2 || start >= end {
        return 0;
    }
    let start = start.max(2);
    let limit = (end as f64).sqrt() as u64 + 1;
    let mut small = vec![true; limit as usize + 1];
    let mut composite = vec![false; (end - start) as usize];
    for p in 2..=limit {
        if !small[p as usize] {
            continue;
        }
        let mut m = p * p;
        while m <= limit {
            small[m as usize] = false;
            m += p;
        }
        let mut m = (p * p).max(start.div_ceil(p) * p);
        while m < end {
            composite[(m - start) as usize] = true;
            m += p;
        }
    }
    composite.iter().filter(|&&c| !c).count() as u64
}

fn run(input: &[u8]) -> Vec<u8> {
    let index = u64::from(u32::from_le_bytes(
        input[..4].try_into().expect("unit index"),
    ));
    let block = u64::from_le_bytes(input[4..12].try_into().expect("block size"));
    primes_in(index * block, (index + 1) * block)
        .to_le_bytes()
        .to_vec()
}

swarm_program::program!(run);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_known_values() {
        assert_eq!(primes_in(0, 10), 4);
        assert_eq!(primes_in(0, 100), 25);
        assert_eq!(primes_in(0, 1_000_000), 78_498);
        // Blocks add up.
        let blocks: u64 = (0..10)
            .map(|i| primes_in(i * 100_000, (i + 1) * 100_000))
            .sum();
        assert_eq!(blocks, 78_498);
        assert_eq!(primes_in(2, 3), 1);
        assert_eq!(primes_in(90, 97), 0);
        assert_eq!(primes_in(90, 98), 1);
    }
}
