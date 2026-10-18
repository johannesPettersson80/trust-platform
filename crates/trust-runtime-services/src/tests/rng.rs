//! A small deterministic random number generator for the contract tests' generated cases: a case
//! that fails is reproduced by its seed.

/// The `SplitMix64` generator (Steele, Lea and Flood, 2014).
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^ (mixed >> 31)
    }

    /// A number in `0..bound`, for a bound above zero.
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).expect("a usize fits a u64");
        usize::try_from(self.next_u64() % bound).expect("a number below a usize fits a usize")
    }

    /// A number in `low..=high`.
    pub(crate) fn between(&mut self, low: usize, high: usize) -> usize {
        low + self.below(high - low + 1)
    }

    /// True once in `times` on average.
    pub(crate) fn one_in(&mut self, times: usize) -> bool {
        self.below(times) == 0
    }

    /// One of `items`, which is not empty.
    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}
