//! The pending flags of a generated runtime: one bit per state that has a
//! completion or an `#[always]` to serve (spec 5.9, 5.10).

/// A set of flags packed into `WORDS` machine words, one bit per flag.
///
/// The generated runtime keeps one bit for every state that can complete or
/// has an `#[always]` handler. [`any`](Self::any) is the check the step
/// algorithm makes on every drain iteration: with up to 64 flags it is one
/// comparison of one word, and each further 64 flags adds one word to the
/// check, never a branch per state.
///
/// ```
/// use estado_core::PendingFlags;
///
/// let mut flags = PendingFlags::<1>::NEW;
/// assert!(!flags.any());
/// flags.set(3);
/// assert!(flags.any());
/// assert!(flags.take(3));
/// assert!(!flags.take(3));
/// assert!(!flags.any());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingFlags<const WORDS: usize> {
    words: [u64; WORDS],
}

impl<const WORDS: usize> PendingFlags<WORDS> {
    /// No flag set; `const` so that `Machine::new` is a `const fn`.
    pub const NEW: Self = Self { words: [0; WORDS] };

    /// Sets flag `bit`.
    ///
    /// # Panics
    ///
    /// If `bit` is not below `WORDS * 64`.
    #[inline(always)]
    pub fn set(&mut self, bit: usize) {
        self.words[bit / 64] |= 1 << (bit % 64);
    }

    /// Clears flag `bit`.
    ///
    /// # Panics
    ///
    /// If `bit` is not below `WORDS * 64`.
    #[inline(always)]
    pub fn clear(&mut self, bit: usize) {
        self.words[bit / 64] &= !(1 << (bit % 64));
    }

    /// Clears flag `bit` and tells whether it was set.
    ///
    /// # Panics
    ///
    /// If `bit` is not below `WORDS * 64`.
    #[inline(always)]
    pub fn take(&mut self, bit: usize) -> bool {
        let mask = 1_u64 << (bit % 64);
        let word = &mut self.words[bit / 64];
        let was_set = *word & mask != 0;
        *word &= !mask;
        was_set
    }

    /// Whether any flag is set.
    #[inline(always)]
    pub fn any(&self) -> bool {
        self.words.iter().any(|&word| word != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::PendingFlags;

    #[test]
    fn flags_across_word_boundaries_are_independent() {
        let mut flags = PendingFlags::<3>::NEW;
        for bit in [0, 63, 64, 127, 128, 191] {
            assert!(!flags.any());
            flags.set(bit);
            assert!(flags.any());
            for other in [0, 63, 64, 127, 128, 191] {
                assert_eq!(flags.take(other), other == bit);
            }
        }
        assert!(!flags.any());
    }

    #[test]
    fn any_stays_set_until_the_last_flag_is_cleared() {
        let mut flags = PendingFlags::<2>::NEW;
        flags.set(5);
        flags.set(70);
        flags.clear(5);
        assert!(flags.any());
        flags.clear(70);
        assert!(!flags.any());
    }

    #[test]
    fn setting_twice_and_clearing_twice_is_idempotent() {
        let mut flags = PendingFlags::<1>::NEW;
        flags.set(9);
        flags.set(9);
        assert!(flags.take(9));
        assert!(!flags.take(9));
        flags.clear(9);
        assert!(!flags.any());
    }
}
