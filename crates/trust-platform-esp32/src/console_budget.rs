//! One console record's shared byte allowance and microsecond drain deadline.
#[cfg(any(test, all(target_arch = "riscv32", target_os = "none")))]
use crate::profile::{RECORD_BYTES, RECORD_DEADLINE_US};

/// Why a bounded console record stopped. A stopped record may have a written prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleError {
    /// Formatting attempted more than 512 bytes in one record.
    RecordTooLong,
    /// The complete record did not drain within 100 ms of SYSTIMER time.
    Timeout,
}

#[cfg(any(test, all(target_arch = "riscv32", target_os = "none")))]
pub(crate) struct RecordBudget {
    start_us: u64,
    remaining: usize,
}
#[cfg(any(test, all(target_arch = "riscv32", target_os = "none")))]
impl RecordBudget {
    pub(crate) fn new(start_us: u64) -> Self {
        Self {
            start_us,
            remaining: RECORD_BYTES,
        }
    }
    pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), ConsoleError> {
        if bytes > self.remaining {
            return Err(ConsoleError::RecordTooLong);
        }
        self.remaining -= bytes;
        Ok(())
    }
    pub(crate) fn check_time(&self, now_us: u64) -> Result<(), ConsoleError> {
        if now_us.saturating_sub(self.start_us) >= RECORD_DEADLINE_US {
            Err(ConsoleError::Timeout)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_fragments_share_one_byte_limit_and_one_deadline() {
        let start = 1_000_000_u64;
        let mut record = RecordBudget::new(start);
        for size in [3, 4, 200, 200, 105] {
            record.reserve(size).unwrap();
        }
        assert_eq!(record.reserve(1), Err(ConsoleError::RecordTooLong));
        record.reserve(0).unwrap();
        assert_eq!(record.check_time(start + RECORD_DEADLINE_US - 1), Ok(()));
        assert_eq!(
            record.check_time(start + RECORD_DEADLINE_US),
            Err(ConsoleError::Timeout)
        );
        assert_eq!(
            record.check_time(start + RECORD_DEADLINE_US + 1),
            Err(ConsoleError::Timeout)
        );
        // A new transaction resets allowances; another fragment never does.
        let mut next = RecordBudget::new(start + 1_000_000);
        next.reserve(512).unwrap();
        assert_eq!(next.check_time(start + 1_000_000), Ok(()));
    }

    #[test]
    fn rejected_fragment_does_not_consume_or_reset_remaining_allowance() {
        let mut record = RecordBudget::new(10);
        record.reserve(500).unwrap();
        assert_eq!(record.reserve(13), Err(ConsoleError::RecordTooLong));
        record.reserve(12).unwrap();
        assert_eq!(record.reserve(1), Err(ConsoleError::RecordTooLong));
    }

    #[test]
    fn deadline_is_measured_from_the_record_start_only() {
        let record = RecordBudget::new(5_000);
        assert_eq!(record.check_time(4_000), Ok(()));
        assert_eq!(record.check_time(5_000 + RECORD_DEADLINE_US - 1), Ok(()));
    }
}
