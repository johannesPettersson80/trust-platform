//! The allocation count of a scan (specification 34, "The allocation count", ARCH-05 and MEM-09;
//! checklist C1-19f).
//!
//! The platform counts the allocator operations of the thread that runs the scans
//! ([`ExecutionServices::allocator_operations`]). The engine reads the count when a scan begins and
//! when it ends, and around every call it makes into a service during the scan, so that the
//! operations it makes itself are told apart from the services'.

use super::*;

/// The engine's allocation count of the scan that runs and of the last one.
#[derive(Debug, Default)]
pub(super) struct ScanAllocations {
    /// The platform's count when the scan began; `None` between scans and on a platform that
    /// does not count.
    began: Option<u64>,
    /// The operations made inside the calls into services during the scan that runs.
    services: u64,
    /// The operations the engine made itself in the last scan; `None` when the platform does not
    /// count them.
    last: Option<u64>,
}

impl ScanAllocations {
    /// The operations the engine made itself in the last scan.
    pub(super) fn last(&self) -> Option<u64> {
        self.last
    }
}

impl EngineState<'_> {
    /// Begin counting a scan.
    pub(super) fn begin_allocation_count(&mut self) {
        let count = &mut self.data.resources.scan_allocations;
        count.began = self.services.allocator_operations();
        count.services = 0;
    }

    /// Count the operations since `before`, a reading taken before a call into a service, as the
    /// services'. Without a reading the count is not read again, so a statement hook that no
    /// debugger uses costs no reading.
    pub(super) fn count_service_operations(&mut self, before: Option<u64>) {
        let Some(before) = before else {
            return;
        };
        let Some(after) = self.services.allocator_operations() else {
            return;
        };
        let count = &mut self.data.resources.scan_allocations;
        count.services = count.services.saturating_add(after.saturating_sub(before));
    }

    /// End counting a scan: the engine's operations are the scan's less the services'.
    pub(super) fn end_allocation_count(&mut self) {
        let ended = self.services.allocator_operations();
        let count = &mut self.data.resources.scan_allocations;
        count.last = match (count.began.take(), ended) {
            (Some(began), Some(ended)) => {
                Some(ended.saturating_sub(began).saturating_sub(count.services))
            }
            _ => None,
        };
    }
}
