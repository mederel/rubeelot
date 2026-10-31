/// A source of bounded random integers.
pub trait RandomSource {
    /// Returns a value in `0..bound`; `bound` must be positive.
    fn next_int(&mut self, bound: i32) -> i32;
}

const GAMMA: i64 = 0x9E3779B97F4A7C15_u64 as i64;

/// SplitMix64, a small, fast generator with a 64-bit state: every seed gives a different, reproducible
/// sequence.
pub struct SplitMix64 {
    seed: i64,
}

impl SplitMix64 {
    pub fn new(seed: i64) -> Self {
        Self { seed }
    }

    fn next_seed(&mut self) -> i64 {
        self.seed = self.seed.wrapping_add(GAMMA);
        self.seed
    }

    pub fn next_long(&mut self) -> i64 {
        let mut z = self.next_seed() as u64;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        (z ^ (z >> 31)) as i64
    }

    fn raw_int(&mut self) -> i32 {
        let mut z = self.next_seed() as u64;
        z = (z ^ (z >> 33)).wrapping_mul(0x62A9D9ED799705F5);
        ((z ^ (z >> 28)).wrapping_mul(0xCB24D0A5C88C35B3) >> 32) as i32
    }
}

impl RandomSource for SplitMix64 {
    fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.raw_int();
        let m = bound.wrapping_sub(1);
        if bound & m == 0 {
            return r & m;
        }
        let mut u = ((r as u32) >> 1) as i32;
        loop {
            r = u % bound;
            if u.wrapping_add(m).wrapping_sub(r) >= 0 {
                return r;
            }
            u = ((self.raw_int() as u32) >> 1) as i32;
        }
    }
}

/// Fisher-Yates: shuffles the items in place, from the last one down.
pub fn shuffle<T>(items: &mut [T], rng: &mut dyn RandomSource) {
    for i in (2..=items.len()).rev() {
        items.swap(i - 1, rng.next_int(i as i32) as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "bound must be positive")]
    fn split_mix_rejects_a_non_positive_bound() {
        SplitMix64::new(1).next_int(-3);
    }
}
