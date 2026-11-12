//! A tiny deterministic generator, so every run emits the same machine.

/// xorshift64.
pub struct Random(u64);

impl Random {
    /// Seeded generator.
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// The next value.
    pub fn next(&mut self) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as usize
    }

    /// A value below `bound` (which must not be 0).
    pub fn below(&mut self, bound: usize) -> usize {
        self.next() % bound
    }
}
