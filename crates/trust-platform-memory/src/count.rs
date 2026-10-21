//! A 64-bit count that a 32-bit chip reads without a critical section (checklist P1-6;
//! specification 34, "The allocation count").
//!
//! The count lies in two 32-bit words beside a mark. The writer, which runs in a critical section so
//! that no other writer interleaves, makes the mark odd, writes the words and makes the mark even
//! again. A reader reads the mark, the words and the mark again, until both marks are the same even
//! number, so it never takes the words of two counts, even when an interrupt writes between its
//! reads.

use core::sync::atomic::{fence, AtomicU32, Ordering};

/// A count of 64 bits in two words and a mark (module documentation).
pub(crate) struct Count64 {
    mark: AtomicU32,
    low: AtomicU32,
    high: AtomicU32,
}

impl Count64 {
    /// A count of zero.
    pub(crate) const fn new() -> Self {
        Self {
            mark: AtomicU32::new(0),
            low: AtomicU32::new(0),
            high: AtomicU32::new(0),
        }
    }

    /// Adds one. The caller holds a critical section, so no other writer interleaves.
    pub(crate) fn increment(&self) {
        let mark = self.mark.load(Ordering::Relaxed);
        self.mark.store(mark.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        let low = self.low.load(Ordering::Relaxed).wrapping_add(1);
        if low == 0 {
            let high = self.high.load(Ordering::Relaxed).wrapping_add(1);
            self.high.store(high, Ordering::Relaxed);
        }
        self.low.store(low, Ordering::Relaxed);
        self.mark.store(mark.wrapping_add(2), Ordering::Release);
    }

    /// The count, read between two equal even marks.
    pub(crate) fn read(&self) -> u64 {
        loop {
            let before = self.mark.load(Ordering::Acquire);
            let low = self.low.load(Ordering::Relaxed);
            let high = self.high.load(Ordering::Relaxed);
            fence(Ordering::Acquire);
            let after = self.mark.load(Ordering::Relaxed);
            if before == after && before.is_multiple_of(2) {
                return u64::from(low) | (u64::from(high) << 32);
            }
            core::hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec::Vec;

    /// A count of `value`, as `value` increments from zero would leave it.
    fn count_at(value: u64) -> Count64 {
        let count = Count64::new();
        let [low, high] = [value & u64::from(u32::MAX), value >> 32]
            .map(|word| u32::try_from(word).unwrap_or(u32::MAX));
        count.low.store(low, Ordering::Relaxed);
        count.high.store(high, Ordering::Relaxed);
        count
    }

    #[test]
    fn a_count_carries_into_its_high_word() {
        // Specification 34, "The allocation count": the count is never reset and does not wrap on a
        // 32-bit chip. Worked by hand: 0xFFFF_FFFE counts on to 0xFFFF_FFFF, 0x1_0000_0000 and
        // 0x1_0000_0001. Catches: a count that wraps at 2^32, or a carry that is lost or taken
        // twice.
        let count = count_at(0xFFFF_FFFE);
        assert_eq!(count.read(), 0xFFFF_FFFE);
        count.increment();
        assert_eq!(count.read(), 0xFFFF_FFFF);
        count.increment();
        assert_eq!(count.read(), 0x1_0000_0000);
        count.increment();
        assert_eq!(count.read(), 0x1_0000_0001);
    }

    #[test]
    fn a_reader_never_takes_the_words_of_two_counts() {
        // The module's rule: a read while the count is written across its carry is the count before
        // or after a write, never the low word of one and the high word of another. One writer, as
        // a chip's critical section leaves one, counts 1,024 times from 0xFFFF_FF00, past 2^32 at
        // its 256th count, while a reader reads until it sees the last count, 0x1_0000_0300. Every
        // read lies from 0xFFFF_FF00 to 0x1_0000_0300 and none is less than the one before.
        // Catches: a read of the new low word with the old high word (0x0000_00xx) or the old low
        // word with the new high word (0x1_FFFF_FFxx).
        let count = count_at(0xFFFF_FF00);
        let mut reads = Vec::new();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..1_024 {
                    count.increment();
                }
            });
            loop {
                let read = count.read();
                reads.push(read);
                if read == 0x1_0000_0300 {
                    break;
                }
            }
        });
        assert!(
            reads
                .iter()
                .all(|read| (0xFFFF_FF00..=0x1_0000_0300).contains(read)),
            "a read outside the counts written"
        );
        assert!(reads.is_sorted(), "a read less than the one before");
    }
}
