//! One UART record's shared byte allowance and wrapping cycle deadline.
#[cfg(any(test, all(target_arch = "arm", target_os = "none")))]
use crate::SYSCLK_HZ;

/// Why a bounded serial record stopped. A stopped record may have a written prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleError {
    /// Formatting attempted more than 512 bytes in one record.
    RecordTooLong,
    /// The complete record did not drain within 100ms of configured core cycles.
    Timeout,
    /// The HAL reported a transmit error.
    Peripheral,
}

#[cfg(any(test, all(target_arch = "arm", target_os = "none")))]
pub(crate) struct RecordBudget {
    start: u32,
    remaining: usize,
}
#[cfg(any(test, all(target_arch = "arm", target_os = "none")))]
impl RecordBudget {
    pub(crate) fn new(start: u32) -> Self {
        Self {
            start,
            remaining: 512,
        }
    }
    pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), ConsoleError> {
        if bytes > self.remaining {
            return Err(ConsoleError::RecordTooLong);
        }
        self.remaining -= bytes;
        Ok(())
    }
    pub(crate) fn check_time(&self, now: u32) -> Result<(), ConsoleError> {
        if now.wrapping_sub(self.start) >= SYSCLK_HZ / 10 {
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
        let start = u32::MAX - 100;
        let mut record = RecordBudget::new(start);
        for size in [3, 4, 200, 200, 105] {
            record.reserve(size).unwrap();
        }
        assert_eq!(record.reserve(1), Err(ConsoleError::RecordTooLong));
        record.reserve(0).unwrap();
        assert_eq!(
            record.check_time(start.wrapping_add(SYSCLK_HZ / 10 - 1)),
            Ok(())
        );
        assert_eq!(
            record.check_time(start.wrapping_add(SYSCLK_HZ / 10)),
            Err(ConsoleError::Timeout)
        );
        assert_eq!(
            record.check_time(start.wrapping_add(SYSCLK_HZ / 10 + 1)),
            Err(ConsoleError::Timeout)
        );
        // A new transaction resets allowances; another fragment never does.
        let mut next = RecordBudget::new(start.wrapping_add(SYSCLK_HZ));
        next.reserve(512).unwrap();
        assert_eq!(next.check_time(start.wrapping_add(SYSCLK_HZ)), Ok(()));
    }

    #[test]
    fn rejected_fragment_does_not_consume_or_reset_remaining_allowance() {
        let mut record = RecordBudget::new(10);
        record.reserve(500).unwrap();
        assert_eq!(record.reserve(13), Err(ConsoleError::RecordTooLong));
        record.reserve(12).unwrap();
        assert_eq!(record.reserve(1), Err(ConsoleError::RecordTooLong));
    }
}
