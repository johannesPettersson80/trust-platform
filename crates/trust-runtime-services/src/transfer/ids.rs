//! Transfer ids (specification 34, PORT-05): sixteen hexadecimal digits whose high 32 bits are the
//! boot number and whose low 32 bits are a sequence within the boot. An id never repeats within a
//! boot or across boots, so an old id never names a transfer of a new boot.

use core::fmt;

/// The id of one upload or held reply, written as 16 lowercase hexadecimal digits: the boot
/// number, then the sequence within the boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferId {
    boot: u32,
    sequence: u32,
}

impl TransferId {
    /// The id of `sequence` within `boot`.
    #[must_use]
    pub const fn new(boot: u32, sequence: u32) -> Self {
        Self { boot, sequence }
    }
}

impl fmt::Display for TransferId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:08x}{:08x}", self.boot, self.sequence)
    }
}

/// The ids of one boot, counted from 1. The sequence only moves forward, so no id repeats.
#[derive(Debug)]
pub struct IdSequence {
    boot: u32,
    last: u32,
}

impl IdSequence {
    /// The ids of `boot`, none of them used yet.
    #[must_use]
    pub const fn new(boot: u32) -> Self {
        Self { boot, last: 0 }
    }

    /// The next id of the boot, or `None` once every sequence number up to `u32::MAX` is used.
    pub fn next_id(&mut self) -> Option<TransferId> {
        let sequence = self.last.checked_add(1)?;
        self.last = sequence;
        Some(TransferId::new(self.boot, sequence))
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::IdSequence;

    #[test]
    fn the_first_ids_of_a_boot_carry_the_boot_high_and_the_sequence_low() {
        // Spec 34 PORT-05: the boot number is the high 32 bits, the sequence the low 32 bits, so
        // the first two ids of boot 1 are these 16 digits. Catches a boot and sequence swapped,
        // or a sequence that starts at 0.
        let mut ids = IdSequence::new(1);
        let first = ids.next_id().expect("boot 1 has ids");
        let second = ids.next_id().expect("boot 1 has more ids");
        assert_eq!(first.to_string(), "0000000100000001");
        assert_eq!(second.to_string(), "0000000100000002");
    }

    #[test]
    fn the_boot_has_no_id_after_its_last_sequence_number() {
        // Spec 34 PORT-05: ids never repeat within a boot. Catches a sequence that wraps to 1
        // after 0xffffffff and hands out an id it already gave.
        let mut ids = IdSequence {
            boot: 1,
            last: 0xffff_fffe,
        };
        let last = ids.next_id().expect("sequence 0xffffffff is the last id");
        assert_eq!(last.to_string(), "00000001ffffffff");
        assert_eq!(ids.next_id(), None);
        assert_eq!(ids.next_id(), None);
    }
}
