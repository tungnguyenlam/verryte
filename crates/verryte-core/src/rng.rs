//! Seeded random number generator for reproducible game behavior.
//!
//! Terminal games and roguelikes need randomness that can be replayed, tested,
//! and driven by agents. [`Rng`] provides a simple xorshift64 implementation
//! that is fast, deterministic, and requires no external dependencies.
//!
//! Use it as a resource in the ECS [`World`](crate::world::World) so systems
//! can draw random values while keeping the seed observable and controllable.
//!
//! # Example
//!
//! ```ignore
//! let mut rng = Rng::seed(42);
//! let roll = rng.roll(1, 6); // deterministic 1-6
//! let coin = rng.flip();     // deterministic true/false
//! ```

/// A simple xorshift64 pseudo-random number generator.
///
/// This RNG is fast, deterministic, and reproducible. Two instances with the
/// same seed will produce identical sequences. This makes it suitable for:
///
/// - Replay systems that need deterministic re-simulation
/// - Agent testing where behavior must be reproducible
/// - Seeded dungeon generation
/// - Any game system that needs randomness with a known seed
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a new RNG with the given seed.
    ///
    /// A seed of 0 is replaced with 1 to avoid the xorshift64 degenerate case.
    pub fn seed(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Create a new RNG with a time-based seed.
    ///
    /// This is useful for non-reproducible gameplay while still allowing the
    /// seed to be logged for debugging.
    pub fn from_time() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self::seed(duration.as_millis() as u64)
    }

    /// Return the current seed value. Useful for logging or replay recording.
    pub fn seed_value(&self) -> u64 {
        self.state
    }

    /// Generate the next u64 value in the sequence.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Generate a u32 value in the range `0..max` (exclusive).
    ///
    /// Returns 0 if `max` is 0.
    pub fn next_u32(&mut self, max: u32) -> u32 {
        if max == 0 {
            return 0;
        }
        (self.next_u64() % (max as u64)) as u32
    }

    /// Generate a value in the range `min..=max` (inclusive).
    ///
    /// Returns `min` if `max < min`.
    pub fn roll(&mut self, min: u32, max: u32) -> u32 {
        if max < min {
            return min;
        }
        min + self.next_u32(max - min + 1)
    }

    /// Generate a random boolean with roughly 50/50 probability.
    pub fn flip(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }

    /// Generate a random boolean with the given probability (0.0 to 1.0).
    ///
    /// Returns `true` with probability `p`. Values outside 0.0–1.0 are clamped.
    pub fn chance(&mut self, p: f64) -> bool {
        let clamped = p.clamp(0.0, 1.0);
        let threshold = (clamped * (u64::MAX as f64)) as u64;
        self.next_u64() < threshold
    }

    /// Pick a random element from a slice.
    ///
    /// Returns `None` if the slice is empty.
    pub fn pick<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            None
        } else {
            Some(&slice[self.next_u32(slice.len() as u32) as usize])
        }
    }

    /// Pick a random element from an iterator.
    ///
    /// Uses reservoir sampling (single item) so it works on any iterator
    /// without collecting into a Vec first. Returns `None` if the iterator
    /// is empty.
    pub fn pick_range<T, I>(&mut self, iter: I) -> Option<T>
    where
        I: IntoIterator<Item = T>,
    {
        let mut iter = iter.into_iter();
        let mut result = iter.next()?;
        let mut count = 1;
        for item in iter {
            count += 1;
            if self.next_u32(count) == 0 {
                result = item;
            }
        }
        Some(result)
    }

    /// Pick a random index in the range `0..len`.
    ///
    /// Returns `None` if `len` is 0.
    pub fn pick_index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            None
        } else {
            Some(self.next_u32(len as u32) as usize)
        }
    }

    /// Pick a random element weighted by the given weights.
    ///
    /// Each element's probability is proportional to its weight. Weights
    /// should be positive. Returns `None` if the slice is empty or all
    /// weights are zero.
    pub fn weighted_pick<'a, T>(&mut self, items: &'a [T], weights: &[u32]) -> Option<&'a T> {
        if items.is_empty() || weights.len() != items.len() {
            return None;
        }
        let total: u64 = weights.iter().map(|&w| w as u64).sum();
        if total == 0 {
            return None;
        }
        let mut roll = self.next_u64() % total;
        for (item, &weight) in items.iter().zip(weights) {
            if roll < weight as u64 {
                return Some(item);
            }
            roll -= weight as u64;
        }
        // Should not reach here, but return last item as fallback.
        items.last()
    }

    /// Shuffle a slice in place using Fisher-Yates.
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        let len = slice.len();
        for i in (1..len).rev() {
            let j = self.next_u32((i + 1) as u32) as usize;
            slice.swap(i, j);
        }
    }

    /// Shuffle a sub-range `[start, end)` of a slice in place using Fisher-Yates.
    ///
    /// Clamps `start` and `end` to valid bounds. Does nothing if the range
    /// has fewer than 2 elements.
    pub fn shuffle_range<T>(&mut self, slice: &mut [T], start: usize, end: usize) {
        let len = slice.len();
        if start >= end || end <= start + 1 {
            return;
        }
        let lo = start.min(len);
        let hi = end.min(len);
        for i in (lo + 1..hi).rev() {
            let j = self.next_u32((i - lo + 1) as u32) as usize + lo;
            slice.swap(i, j);
        }
    }

    /// Generate a random `f64` in the range `0.0..1.0`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }

    /// Generate a random `f64` from a normal (Gaussian) distribution with the
    /// given mean and standard deviation.
    ///
    /// Uses the Box-Muller transform. Values can fall outside any range —
    /// clamp or reject as needed for your use case.
    ///
    /// Useful for natural-looking randomness: stat distributions, damage
    /// variance, spawn clustering, etc.
    pub fn gaussian(&mut self, mean: f64, std_dev: f64) -> f64 {
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        mean + std_dev * z
    }

    /// Generate a random integer from a normal distribution, clamped to
    /// `[min, max]` (inclusive).
    ///
    /// Useful for bounded Gaussian randomness like stat rolls or damage.
    pub fn gaussian_int(&mut self, mean: f64, std_dev: f64, min: i64, max: i64) -> i64 {
        let val = self.gaussian(mean, std_dev);
        val.round().clamp(min as f64, max as f64) as i64
    }
}

impl Default for Rng {
    fn default() -> Self {
        Self::seed(42)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_same_sequence() {
        let mut rng1 = Rng::seed(12345);
        let mut rng2 = Rng::seed(12345);
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn different_seeds_produce_different_sequences() {
        let mut rng1 = Rng::seed(1);
        let mut rng2 = Rng::seed(2);
        let mut any_different = false;
        for _ in 0..100 {
            if rng1.next_u64() != rng2.next_u64() {
                any_different = true;
                break;
            }
        }
        assert!(any_different);
    }

    #[test]
    fn seed_zero_is_replaced_with_one() {
        let mut rng_zero = Rng::seed(0);
        let mut rng_one = Rng::seed(1);
        assert_eq!(rng_zero.next_u64(), rng_one.next_u64());
    }

    #[test]
    fn next_u32_respects_max() {
        let mut rng = Rng::seed(42);
        for _ in 0..1000 {
            let val = rng.next_u32(10);
            assert!(val < 10);
        }
    }

    #[test]
    fn next_u32_returns_zero_for_max_zero() {
        let mut rng = Rng::seed(42);
        assert_eq!(rng.next_u32(0), 0);
    }

    #[test]
    fn roll_respects_range() {
        let mut rng = Rng::seed(42);
        for _ in 0..1000 {
            let val = rng.roll(5, 15);
            assert!(val >= 5 && val <= 15);
        }
    }

    #[test]
    fn roll_returns_min_when_max_less_than_min() {
        let mut rng = Rng::seed(42);
        assert_eq!(rng.roll(10, 5), 10);
    }

    #[test]
    fn flip_produces_both_values() {
        let mut rng = Rng::seed(42);
        let mut true_count = 0;
        for _ in 0..100 {
            if rng.flip() {
                true_count += 1;
            }
        }
        // Should not be all true or all false.
        assert!(true_count > 0 && true_count < 100);
    }

    #[test]
    fn chance_always_true_at_1_0() {
        let mut rng = Rng::seed(42);
        for _ in 0..100 {
            assert!(rng.chance(1.0));
        }
    }

    #[test]
    fn chance_always_false_at_0_0() {
        let mut rng = Rng::seed(42);
        for _ in 0..100 {
            assert!(!rng.chance(0.0));
        }
    }

    #[test]
    fn pick_returns_none_for_empty_slice() {
        let mut rng = Rng::seed(42);
        let empty: &[i32] = &[];
        assert!(rng.pick(empty).is_none());
    }

    #[test]
    fn pick_returns_element_from_non_empty() {
        let mut rng = Rng::seed(42);
        let items = [10, 20, 30];
        let picked = rng.pick(&items).unwrap();
        assert!(*picked == 10 || *picked == 20 || *picked == 30);
    }

    #[test]
    fn pick_index_returns_valid_index() {
        let mut rng = Rng::seed(42);
        for len in 1..=10 {
            let idx = rng.pick_index(len).unwrap();
            assert!(idx < len);
        }
        assert!(rng.pick_index(0).is_none());
    }

    #[test]
    fn shuffle_reorders_elements() {
        let mut rng = Rng::seed(42);
        let mut items: Vec<i32> = (0..20).collect();
        let original = items.clone();
        rng.shuffle(&mut items);
        // Should be a permutation (same elements, different order).
        items.sort();
        assert_eq!(items, original);
    }

    #[test]
    fn shuffle_with_seed_1_is_deterministic() {
        let mut rng1 = Rng::seed(99);
        let mut rng2 = Rng::seed(99);
        let mut items1: Vec<i32> = (0..10).collect();
        let mut items2: Vec<i32> = (0..10).collect();
        rng1.shuffle(&mut items1);
        rng2.shuffle(&mut items2);
        assert_eq!(items1, items2);
    }

    #[test]
    fn next_f64_in_range() {
        let mut rng = Rng::seed(42);
        for _ in 0..100 {
            let val = rng.next_f64();
            assert!(val >= 0.0 && val < 1.0);
        }
    }

    #[test]
    fn default_seed_is_deterministic() {
        let mut rng1 = Rng::default();
        let mut rng2 = Rng::default();
        for _ in 0..10 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn rng_is_clone() {
        let mut rng1 = Rng::seed(42);
        let _ = rng1.next_u64();
        let mut rng2 = rng1.clone();
        assert_eq!(rng1.next_u64(), rng2.next_u64());
    }

    #[test]
    fn weighted_pick_returns_none_for_empty() {
        let mut rng = Rng::seed(42);
        let items: &[i32] = &[];
        let weights: &[u32] = &[];
        assert!(rng.weighted_pick(items, weights).is_none());
    }

    #[test]
    fn weighted_pick_returns_none_for_mismatched_lengths() {
        let mut rng = Rng::seed(42);
        assert!(rng.weighted_pick(&[1, 2], &[10]).is_none());
    }

    #[test]
    fn weighted_pick_returns_none_for_all_zero_weights() {
        let mut rng = Rng::seed(42);
        assert!(rng.weighted_pick(&[1, 2, 3], &[0, 0, 0]).is_none());
    }

    #[test]
    fn weighted_pick_respects_weights() {
        let mut rng = Rng::seed(42);
        let items = ["common", "rare"];
        let weights = [90, 10];
        let mut common_count = 0;
        for _ in 0..1000 {
            if *rng.weighted_pick(&items, &weights).unwrap() == "common" {
                common_count += 1;
            }
        }
        // "common" should be picked roughly 90% of the time.
        assert!(
            common_count > 700,
            "common should be picked >700 times, got {common_count}"
        );
    }

    #[test]
    fn weighted_pick_is_deterministic() {
        let mut rng1 = Rng::seed(123);
        let mut rng2 = Rng::seed(123);
        let items = ["a", "b", "c"];
        let weights = [1, 2, 3];
        for _ in 0..50 {
            assert_eq!(
                rng1.weighted_pick(&items, &weights),
                rng2.weighted_pick(&items, &weights)
            );
        }
    }

    #[test]
    fn weighted_pick_with_single_item() {
        let mut rng = Rng::seed(42);
        let result = rng.weighted_pick(&["only"], &[5]);
        assert_eq!(result, Some(&"only"));
    }

    #[test]
    fn gaussian_is_deterministic() {
        let mut rng1 = Rng::seed(99);
        let mut rng2 = Rng::seed(99);
        for _ in 0..50 {
            assert_eq!(rng1.gaussian(0.0, 1.0), rng2.gaussian(0.0, 1.0));
        }
    }

    #[test]
    fn gaussian_mean_is_approximately_correct() {
        let mut rng = Rng::seed(42);
        let mut sum = 0.0;
        let n = 10000;
        for _ in 0..n {
            sum += rng.gaussian(50.0, 5.0);
        }
        let mean = sum / n as f64;
        assert!(
            (mean - 50.0).abs() < 1.0,
            "mean should be close to 50, got {mean}"
        );
    }

    #[test]
    fn gaussian_int_respects_bounds() {
        let mut rng = Rng::seed(42);
        for _ in 0..1000 {
            let val = rng.gaussian_int(50.0, 20.0, 0, 100);
            assert!(val >= 0 && val <= 100);
        }
    }

    #[test]
    fn gaussian_int_is_deterministic() {
        let mut rng1 = Rng::seed(77);
        let mut rng2 = Rng::seed(77);
        for _ in 0..50 {
            assert_eq!(
                rng1.gaussian_int(10.0, 3.0, 0, 20),
                rng2.gaussian_int(10.0, 3.0, 0, 20)
            );
        }
    }

    #[test]
    fn pick_range_returns_none_for_empty() {
        let mut rng = Rng::seed(42);
        let empty: std::ops::Range<i32> = 0..0;
        assert!(rng.pick_range(empty).is_none());
    }

    #[test]
    fn pick_range_returns_element_from_non_empty() {
        let mut rng = Rng::seed(42);
        let picked: Option<i32> = rng.pick_range(1..=5);
        assert!(picked.is_some());
        let val = picked.unwrap();
        assert!(val >= 1 && val <= 5);
    }

    #[test]
    fn pick_range_is_deterministic() {
        let mut rng1 = Rng::seed(99);
        let mut rng2 = Rng::seed(99);
        for _ in 0..50 {
            let a: Option<i32> = rng1.pick_range(1..=10);
            let b: Option<i32> = rng2.pick_range(1..=10);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn pick_range_works_with_vec_iter() {
        let mut rng = Rng::seed(42);
        let items: Vec<&str> = vec!["alpha", "beta", "gamma"];
        let picked: Option<&&str> = rng.pick_range(items.iter());
        assert!(picked.is_some());
        assert!(
            *picked.unwrap() == "alpha"
                || *picked.unwrap() == "beta"
                || *picked.unwrap() == "gamma"
        );
    }

    #[test]
    fn shuffle_range_shuffles_sub_range() {
        let mut rng = Rng::seed(42);
        let mut items: Vec<i32> = (0..10).collect();
        rng.shuffle_range(&mut items, 2, 7);

        // Elements outside range should be unchanged.
        assert_eq!(items[0], 0);
        assert_eq!(items[1], 1);
        assert_eq!(items[8], 8);
        assert_eq!(items[9], 9);

        // Elements inside range should be a permutation.
        let mut subrange: Vec<i32> = items[2..7].to_vec();
        subrange.sort();
        assert_eq!(subrange, vec![2, 3, 4, 5, 6]);
    }

    #[test]
    fn shuffle_range_clamps_to_bounds() {
        let mut rng = Rng::seed(42);
        let mut items: Vec<i32> = (0..5).collect();
        rng.shuffle_range(&mut items, 0, 100);

        let mut sorted = items.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn shuffle_range_small_range_is_noop() {
        let mut rng = Rng::seed(42);
        let mut items = vec![1, 2, 3];
        rng.shuffle_range(&mut items, 1, 1);
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[test]
    fn shuffle_range_is_deterministic() {
        let mut rng1 = Rng::seed(77);
        let mut rng2 = Rng::seed(77);
        let mut items1: Vec<i32> = (0..10).collect();
        let mut items2: Vec<i32> = (0..10).collect();
        rng1.shuffle_range(&mut items1, 2, 8);
        rng2.shuffle_range(&mut items2, 2, 8);
        assert_eq!(items1, items2);
    }
}
