//! The allocation count of a scan (specification 34, "The allocation count", ARCH-05 and MEM-09;
//! checklist C1-19f).
//!
//! The platform counts the allocator operations of the thread that runs the scans
//! ([`ExecutionServices::allocator_operations`]). The engine reads the count when a scan begins and
//! when it ends, and around every call it makes into a service during the scan, so that the
//! operations it makes itself are told apart from the services'. The count is kept in 32 bits that
//! wrap, so each reading is one load on a 32-bit chip: the operations between two readings are
//! their wrapping difference, exact because no scan makes 2^32 operations.

use super::*;

/// The allocator operations between two readings of a platform's count, `before` and then
/// `after`: the count is kept in 32 bits that wrap, so they are the readings' wrapping difference,
/// exact while fewer than 2^32 operations lie between them, which no scan makes (specification 34,
/// "The allocation count").
#[must_use]
pub const fn operations_between(before: u32, after: u32) -> u32 {
    after.wrapping_sub(before)
}

/// The engine's allocation count of the scan that runs and of the last one.
#[derive(Debug, Default)]
pub(super) struct ScanAllocations {
    /// The platform's count when the scan began; `None` between scans and on a platform that
    /// does not count.
    began: Option<u32>,
    /// The operations made inside the calls into services during the scan that runs.
    services: u32,
    /// The operations the engine made itself in the last scan; `None` when the platform does not
    /// count them.
    last: Option<u32>,
    /// The function that gives the platform's count, taken when the scan began
    /// ([`ExecutionServices::allocator_operations_reader`]).
    reader: Option<fn() -> u32>,
}

impl ScanAllocations {
    /// The operations the engine made itself in the last scan.
    pub(super) fn last(&self) -> Option<u64> {
        self.last.map(u64::from)
    }
}

impl EngineState<'_> {
    /// Begin counting a scan.
    pub(super) fn begin_allocation_count(&mut self) {
        self.data.resources.scan_allocations.reader = self.services.allocator_operations_reader();
        let began = self.allocator_operations();
        let count = &mut self.data.resources.scan_allocations;
        count.began = began;
        count.services = 0;
    }

    /// The platform's count of allocator operations, through the function the scan took when the
    /// platform gives one, else through the services.
    pub(super) fn allocator_operations(&self) -> Option<u32> {
        match self.data.resources.scan_allocations.reader {
            Some(read) => Some(read()),
            None => self.services.allocator_operations(),
        }
    }

    /// Count the operations since `before`, a reading taken before a call into a service, as the
    /// services'. Without a reading the count is not read again, so a statement hook that no
    /// debugger uses costs no reading.
    pub(super) fn count_service_operations(&mut self, before: Option<u32>) {
        let Some(before) = before else {
            return;
        };
        let Some(after) = self.allocator_operations() else {
            return;
        };
        let count = &mut self.data.resources.scan_allocations;
        count.services = count
            .services
            .saturating_add(operations_between(before, after));
    }

    /// End counting a scan: the engine's operations are the scan's less the services'.
    pub(super) fn end_allocation_count(&mut self) {
        let ended = self.allocator_operations();
        let count = &mut self.data.resources.scan_allocations;
        count.last = match (count.began.take(), ended) {
            (Some(began), Some(ended)) => {
                Some(operations_between(began, ended).saturating_sub(count.services))
            }
            _ => None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::operations_between;

    #[test]
    fn the_operations_between_two_readings_are_exact_across_the_counts_wrap() {
        // Specification 34, "The allocation count": the count is kept in 32 bits that wrap, and
        // the operations between two readings are their wrapping difference. Worked by hand: from
        // 0xFFFF_FFFE three operations take the count to 0xFFFF_FFFF, 0 and 1, so the readings
        // 0xFFFF_FFFE and 1 lie 3 operations apart; 5 and 9 lie 4 apart, and equal readings none.
        // Catches a saturating difference (0 across the wrap) and a plain one (an overflow).
        assert_eq!(operations_between(0xFFFF_FFFE, 1), 3);
        assert_eq!(operations_between(5, 9), 4);
        assert_eq!(operations_between(7, 7), 0);
    }
}
