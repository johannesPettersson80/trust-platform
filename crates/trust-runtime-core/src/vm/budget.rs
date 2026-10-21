use crate::error::{ExecutionLimitKind, RuntimeError};

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
    limit: core::cell::Cell<usize>,
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
            limit: core::cell::Cell::new(limit),
            remaining: core::cell::Cell::new(limit),
            until_deadline: core::cell::Cell::new(Self::DEADLINE_STRIDE),
            deadline_expired: core::cell::Cell::new(false),
        }
    }

    /// Begin a new construction, engineering operation or scan.
    pub fn reset(&self, limit: usize) {
        self.limit.set(limit);
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
        let Some(remaining) = self.remaining.get().checked_sub(units) else {
            return Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                used: self.limit.get().saturating_sub(self.remaining.get()),
                requested: units,
                limit: self.limit.get(),
            });
        };
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

    /// Charge `units` instructions of one unit each at once, when none of their charges would be
    /// refused or request a deadline sample: the allowance is then left as `units` calls of
    /// [`Self::charge`] with 1 leave it. `false`, with nothing charged, when one of them would be
    /// refused or sample, so the caller charges them one by one (checklist P1-9).
    pub fn charge_run(&self, units: usize) -> bool {
        let remaining = self.remaining.get();
        let until = self.until_deadline.get();
        if units > remaining || units >= until {
            return false;
        }
        self.remaining.set(remaining - units);
        self.until_deadline.set(until - units);
        true
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

    /// A run charged at once leaves the allowance as its unit charges one by one leave it, and is
    /// refused, charging nothing, exactly when one of them would be refused or sample the deadline.
    /// Worked by hand for a fresh allowance of 100 with the stride of 32: a run of 31 leaves 69 and
    /// one unit before the next sample, so a run of 1 there would sample and is refused; after that
    /// sample a run of 31 fits again. Catches a run that skips a deadline sample, overdraws the
    /// allowance, or charges part of itself when refused.
    #[test]
    fn a_run_charges_as_its_units_one_by_one() {
        let budget = ExecutionBudget::new(100);
        assert!(budget.charge_run(31));
        assert_eq!(budget.remaining(), 69);
        assert!(!budget.charge_run(1), "the next unit samples the deadline");
        assert_eq!(budget.remaining(), 69, "a refused run charges nothing");
        assert!(budget.charge(1).unwrap(), "charged alone, the unit samples");
        assert!(budget.charge_run(31));
        assert_eq!(budget.remaining(), 37);
        budget.reset(3);
        assert!(!budget.charge_run(4), "4 units overdraw an allowance of 3");
        assert!(budget.charge_run(3));
        assert_eq!(budget.remaining(), 0);

        // Every allowance, every point of the stride and every run up to 40 units against the run's
        // unit charges one by one.
        for limit in 0..70 {
            for before in 0..40 {
                for units in 0..40 {
                    let at_once = ExecutionBudget::new(limit);
                    let one_by_one = ExecutionBudget::new(limit);
                    for _ in 0..before {
                        let _ = at_once.charge(1);
                        let _ = one_by_one.charge(1);
                    }
                    let start = (at_once.remaining(), at_once.until_deadline.get());
                    let charged = at_once.charge_run(units);
                    let quiet = (0..units).all(|_| one_by_one.charge(1) == Ok(false));
                    assert_eq!(
                        charged, quiet,
                        "limit {limit}, {before} before, run of {units}"
                    );
                    if charged {
                        assert_eq!(
                            (at_once.remaining(), at_once.until_deadline.get()),
                            (one_by_one.remaining(), one_by_one.until_deadline.get())
                        );
                    } else {
                        assert_eq!((at_once.remaining(), at_once.until_deadline.get()), start);
                    }
                }
            }
        }
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

    #[test]
    fn first_refused_work_charge_preserves_used_requested_and_limit() {
        let budget = ExecutionBudget::new(17);
        budget.charge(12).unwrap();
        let error = budget.charge(6).unwrap_err();
        assert_eq!(
            error,
            RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                used: 12,
                requested: 6,
                limit: 17,
            }
        );
        assert_eq!(
            error.stable_code(),
            crate::error::StableErrorCode::RuntimeExecutionTimeout
        );
        assert_eq!(budget.remaining(), 5);
        budget.reset(3);
        assert_eq!(
            budget.charge(4),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                used: 0,
                requested: 4,
                limit: 3,
            })
        );
    }
}
