use crate::error::RuntimeError;

use super::errors::VmTrap;

/// Whether a hosted dispatch entry starts a fresh allowance or joins its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionEntry {
    /// Top-level hosted entry. Portable entries share the enclosing resource budget.
    Root,
    /// A nested call or initializer shares the current allowance.
    Nested,
}

/// The single work allowance shared by an execution entry and every nested helper.
#[derive(Debug)]
pub struct ExecutionBudget {
    remaining: core::cell::Cell<usize>,
    until_deadline: core::cell::Cell<usize>,
    deadline_expired: core::cell::Cell<bool>,
}

impl ExecutionBudget {
    /// Work between cooperative physical-deadline samples.
    pub const DEADLINE_STRIDE: usize = 32;

    /// Establish an allowance; entry boundaries check their deadline explicitly.
    pub const fn new(limit: usize) -> Self {
        Self {
            remaining: core::cell::Cell::new(limit),
            until_deadline: core::cell::Cell::new(Self::DEADLINE_STRIDE),
            deadline_expired: core::cell::Cell::new(false),
        }
    }

    /// Begin a new construction, engineering operation or scan.
    pub fn reset(&self, limit: usize) {
        self.remaining.set(limit);
        self.until_deadline.set(Self::DEADLINE_STRIDE);
        self.deadline_expired.set(false);
    }

    /// Observe an operation's physical deadline at a caller-selected boundary.
    /// Once expired, unwinding reuses the result without sampling the clock again.
    /// Only resetting the enclosing operation clears this latch.
    pub fn observe_deadline(&self, sample: impl FnOnce() -> bool) -> bool {
        if self.deadline_expired.get() {
            return true;
        }
        let expired = sample();
        self.deadline_expired.set(expired);
        expired
    }

    /// Remaining work, for diagnostics and native boundary assertions.
    #[cfg(any(feature = "hir", test))]
    pub fn remaining(&self) -> usize {
        self.remaining.get()
    }

    /// Consume the complete requested work charge, or leave the allowance unchanged.
    /// True requests one physical-deadline sample.
    /// Large aggregate charges do not cause repeated clock calls for one operation.
    pub fn charge(&self, units: usize) -> Result<bool, RuntimeError> {
        let remaining = self
            .remaining
            .get()
            .checked_sub(units)
            .ok_or_else(|| VmTrap::BudgetExceeded.into_runtime_error())?;
        self.remaining.set(remaining);
        let until = self.until_deadline.get();
        if units < until {
            self.until_deadline.set(until - units);
            Ok(false)
        } else {
            let remainder = (units - until) % Self::DEADLINE_STRIDE;
            self.until_deadline.set(Self::DEADLINE_STRIDE - remainder);
            Ok(true)
        }
    }
}

#[cfg(test)]
mod shared_tests {
    use super::*;

    #[test]
    fn work_charges_poll_at_stride_and_do_not_refund_a_failed_charge() {
        let budget = ExecutionBudget::new(100);
        for _ in 0..31 {
            assert!(!budget.charge(1).unwrap());
        }
        assert!(budget.charge(1).unwrap());
        assert!(budget.charge(64).unwrap());
        assert_eq!(budget.remaining(), 4);
        assert!(budget.charge(5).is_err());
        assert_eq!(budget.remaining(), 4);
        assert!(!budget.charge(0).unwrap());
        budget.reset(2);
        assert!(!budget.charge(2).unwrap());
        assert_eq!(budget.remaining(), 0);
        assert!(budget.charge(1).is_err());
    }

    #[test]
    fn deadline_observation_stays_expired_until_the_next_operation_reset() {
        let budget = ExecutionBudget::new(100);
        let calls = core::cell::Cell::new(0);
        for _ in 0..2 {
            assert!(!budget.observe_deadline(|| {
                calls.set(calls.get() + 1);
                false
            }));
        }
        assert!(budget.observe_deadline(|| {
            calls.set(calls.get() + 1);
            true
        }));
        assert_eq!(calls.get(), 3);
        assert!(
            budget.charge(32).unwrap(),
            "expiry does not bypass work accounting"
        );
        assert_eq!(budget.remaining(), 68);
        assert!(budget.observe_deadline(|| panic!("expired operation sampled the clock again")));
        assert!(
            budget.observe_deadline(|| false),
            "later clock changes cannot revive an operation"
        );
        budget.reset(64);
        assert_eq!(budget.remaining(), 64);
        assert!(!budget.charge(31).unwrap());
        assert!(budget.charge(1).unwrap());
        assert!(!budget.observe_deadline(|| {
            calls.set(calls.get() + 1);
            false
        }));
        assert_eq!(calls.get(), 4);
    }
}
