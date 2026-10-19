//! Breakpoint records, hit conditions and hit counts, and the cycle suppression of one scan
//! (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
//!
//! [`count_hit`] is the hit rule as one function. It decides whether a statement's start matches a
//! breakpoint and is not cycle-suppressed, then counts the hit and applies the hit condition to the
//! count. [`observe_hit`] applies it and stores the count. A composing debugger implements
//! [`BreakpointRecord`] for its own record, so every record follows the same rule. The breakpoint's
//! condition expression and its logpoint belong to the composing debugger, which evaluates them
//! after a hit that the rule lets through.

use alloc::collections::BTreeSet;

use super::statement::StatementId;

/// A hit-count condition, applied to the count after the hit is counted (specification 13,
/// "Runtime debug state, mutation, and projection invariants": exact unsigned arithmetic, including
/// at `0` and `u64::MAX`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitCondition {
    /// Stop when the hit count equals the target.
    Equal(u64),
    /// Stop when the hit count is at least the target.
    AtLeast(u64),
    /// Stop when the hit count is greater than the target.
    GreaterThan(u64),
}

impl HitCondition {
    /// Whether the condition holds for `hits`, the hit count after the hit was counted.
    #[must_use]
    pub fn is_met(self, hits: u64) -> bool {
        match self {
            Self::Equal(target) => hits == target,
            Self::AtLeast(target) => hits >= target,
            Self::GreaterThan(target) => hits > target,
        }
    }
}

/// The parts of a breakpoint record that the hit rule reads and writes: the statement it names at
/// its generation, its hit condition and its count. A composing debugger implements this for its
/// own record, so one rule serves every record.
pub trait BreakpointRecord<P> {
    /// The statement the breakpoint names, at its generation.
    fn statement(&self) -> StatementId<P>;

    /// The hit condition, or `None` for a breakpoint that stops at every hit.
    fn hit_condition(&self) -> Option<HitCondition>;

    /// The hits counted so far.
    fn hits(&self) -> u64;

    /// Store the count after a hit that counted.
    fn set_hits(&mut self, hits: u64);
}

/// A breakpoint record of this module: the statement it names at its generation, its hit condition
/// and its hit count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakpoint<P> {
    /// The statement the breakpoint names, at its generation.
    pub id: StatementId<P>,
    /// The hit condition, or `None` for a breakpoint that stops at every hit.
    pub hit_condition: Option<HitCondition>,
    /// The hits counted so far. The count saturates at `u64::MAX` (specification 13).
    pub hits: u64,
}

impl<P: Ord + Copy> Breakpoint<P> {
    /// An unconditional breakpoint on `id` with no hits counted.
    #[must_use]
    pub fn new(id: StatementId<P>) -> Self {
        Self {
            id,
            hit_condition: None,
            hits: 0,
        }
    }
}

impl<P: Ord + Copy> BreakpointRecord<P> for Breakpoint<P> {
    fn statement(&self) -> StatementId<P> {
        self.id
    }

    fn hit_condition(&self) -> Option<HitCondition> {
        self.hit_condition
    }

    fn hits(&self) -> u64 {
        self.hits
    }

    fn set_hits(&mut self, hits: u64) {
        self.hits = hits;
    }
}

/// The ordinary stops taken in the current scan, keyed by statement identity. A stop suppresses its
/// identity for the rest of that scan only, so one breakpoint identity emits at most one stop per
/// scan (specification 13, "Breakpoint evaluation, source resolution, and hook contract").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleSuppression<P> {
    stopped: BTreeSet<StatementId<P>>,
}

impl<P> Default for CycleSuppression<P> {
    fn default() -> Self {
        Self {
            stopped: BTreeSet::new(),
        }
    }
}

impl<P: Ord + Copy> CycleSuppression<P> {
    /// Whether an ordinary stop of `id` was already taken in this scan.
    #[must_use]
    pub fn suppresses(&self, id: &StatementId<P>) -> bool {
        self.stopped.contains(id)
    }

    /// Record an ordinary stop of `id`, which suppresses `id` for the rest of this scan. A logpoint
    /// never stops, so the caller records only an ordinary stop.
    pub fn record_stop(&mut self, id: StatementId<P>) {
        self.stopped.insert(id);
    }

    /// Begin a scan. Every suppression ends, so the next scan may stop again at the same statements
    /// (specification 13).
    pub fn begin_cycle(&mut self) {
        self.stopped.clear();
    }
}

/// A hit that counts at a statement start: the count after it, and whether its hit condition lets
/// it proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountedHit {
    /// The hit count after this hit, saturating at `u64::MAX`.
    pub count: u64,
    /// Whether the hit condition, applied to `count`, lets the hit proceed. It is `true` when the
    /// breakpoint has no hit condition.
    pub passes: bool,
}

/// The hit rule of specification 13's hook contract for one statement start. It reads the record
/// and changes nothing.
///
/// A statement that does not match the record, because it lies in another file or does not start
/// inside the span, counts nothing. So does a matching statement whose ordinary stop `suppressed`
/// already holds in this scan. Both give `None`. Any other match counts: the count is the record's
/// count plus one, saturating at `u64::MAX`, and the hit condition is applied to that incremented
/// count. A hit whose condition fails still counts, with `passes` false.
#[must_use]
pub fn count_hit<P: Ord + Copy, R: BreakpointRecord<P>>(
    record: &R,
    suppressed: impl FnOnce(&StatementId<P>) -> bool,
    file_id: u32,
    start: P,
) -> Option<CountedHit> {
    let id = record.statement();
    if !id.matches(file_id, start) || suppressed(&id) {
        return None;
    }
    let count = record.hits().saturating_add(1);
    let passes = record
        .hit_condition()
        .is_none_or(|condition| condition.is_met(count));
    Some(CountedHit { count, passes })
}

/// Apply the start of one statement to one breakpoint record: [`count_hit`], then store the count.
///
/// `true` means the hit may stop the scan, and the caller then evaluates the breakpoint's condition
/// expression. A hit that the condition rejects keeps its count.
#[must_use]
pub fn observe_hit<P: Ord + Copy, R: BreakpointRecord<P>>(
    record: &mut R,
    suppressed: impl FnOnce(&StatementId<P>) -> bool,
    file_id: u32,
    start: P,
) -> bool {
    let Some(hit) = count_hit(record, suppressed, file_id, start) else {
        return false;
    };
    record.set_hits(hit.count);
    hit.passes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debug::Span;

    /// A breakpoint on the statement at `start..end` of file `file_id`, at `generation`.
    fn breakpoint(file_id: u32, start: u32, end: u32, generation: u64) -> Breakpoint<u32> {
        Breakpoint::new(StatementId {
            file_id,
            span: Span::new(start, end),
            generation,
        })
    }

    #[test]
    fn a_breakpoint_on_a_touching_span_does_not_match() {
        // Specification 13 (hook contract): the span is half-open, so a statement that starts at
        // its end only touches it. Wrong behaviour caught: an inclusive end matches the statement
        // that follows the breakpoint, so the stop comes one statement late.
        let mut after = breakpoint(1, 10, 20, 1);
        assert!(!observe_hit(&mut after, |_| false, 1, 20));
        assert_eq!(after.hits, 0);

        // Wrong behaviour caught: a rule that accepts a statement touching the span from before.
        // The statement 0..10 touches the span at 10, and its start 0 lies outside the span.
        let mut before = breakpoint(1, 10, 20, 1);
        assert!(!observe_hit(&mut before, |_| false, 1, 0));
        assert_eq!(before.hits, 0);
    }

    #[test]
    fn a_breakpoint_matches_a_statement_that_starts_inside_its_span() {
        // Specification 13 (hook contract), the A01 cases of trust-runtime's breakpoint tests: the
        // start is inclusive, so a statement at 5 and one at 9 match the span 5..10, and the
        // enclosing statement that starts at 0 does not. Wrong behaviour caught: an exclusive start
        // misses the breakpoint's own first statement, and an enclosing match stops at its header.
        let mut at_start = breakpoint(1, 5, 10, 1);
        assert!(observe_hit(&mut at_start, |_| false, 1, 5));
        let mut inside = breakpoint(1, 5, 10, 1);
        assert!(observe_hit(&mut inside, |_| false, 1, 9));
        let mut enclosing = breakpoint(1, 5, 10, 1);
        assert!(!observe_hit(&mut enclosing, |_| false, 1, 0));
        assert_eq!(enclosing.hits, 0);
    }

    #[test]
    fn a_breakpoint_does_not_match_a_statement_of_another_file() {
        // Specification 13: a runtime breakpoint matches only statements with its own file ID.
        // Wrong behaviour caught: matching on the span alone stops in another file's statement at
        // the same offsets.
        let mut bp = breakpoint(1, 10, 20, 1);
        assert!(!observe_hit(&mut bp, |_| false, 2, 10));
        assert_eq!(bp.hits, 0);
    }

    #[test]
    fn a_hit_count_saturates_at_the_unsigned_maximum() {
        // Specification 13: the hit counter increments with saturating arithmetic. Wrong behaviour
        // caught: a wrapping increment returns the count from u64::MAX to 0.
        let mut bp = breakpoint(1, 10, 20, 1);
        bp.hits = u64::MAX;
        assert!(observe_hit(&mut bp, |_| false, 1, 10));
        assert_eq!(bp.hits, u64::MAX);
    }

    #[test]
    fn a_cycle_suppressed_hit_does_not_increment_the_hit_count() {
        // Specification 13: the hit counter increments only when the exact stop has not been
        // cycle-suppressed. Wrong behaviour caught: counting the suppressed hit, so a later hit
        // condition sees a count that includes hits that never stopped.
        let mut bp = breakpoint(1, 10, 20, 7);
        let mut suppression: CycleSuppression<u32> = CycleSuppression::default();
        assert!(observe_hit(&mut bp, |id| suppression.suppresses(id), 1, 10));
        suppression.record_stop(bp.id);

        assert!(!observe_hit(
            &mut bp,
            |id| suppression.suppresses(id),
            1,
            10
        ));
        assert_eq!(bp.hits, 1);
    }

    #[test]
    fn a_hit_that_fails_its_hit_condition_still_counts() {
        // Specification 13: a hit whose condition does not stop still retains the hit count, and
        // `Equal(2)` stops at the second hit only. Wrong behaviour caught: a count that resets or
        // is skipped when the condition fails.
        let mut bp = breakpoint(1, 10, 20, 1);
        bp.hit_condition = Some(HitCondition::Equal(2));

        assert!(!observe_hit(&mut bp, |_| false, 1, 10));
        assert_eq!(bp.hits, 1);
        assert!(observe_hit(&mut bp, |_| false, 1, 10));
        assert_eq!(bp.hits, 2);
    }

    #[test]
    fn the_hit_condition_is_applied_to_the_incremented_count() {
        // Specification 13: "Hit conditions are applied to the incremented count". `AtLeast(1)`
        // therefore stops at the first hit, whose count is 1. Wrong behaviour caught: a condition
        // checked before the increment sees 0 at the first hit and never stops.
        let mut bp = breakpoint(1, 10, 20, 1);
        bp.hit_condition = Some(HitCondition::AtLeast(1));

        assert!(observe_hit(&mut bp, |_| false, 1, 10));
        assert_eq!(bp.hits, 1);
    }

    #[test]
    fn a_count_hit_reads_the_next_count_without_counting_it() {
        // Specification 13: the count rises by one when a hit counts, and the hit condition is
        // applied to the new count. A read of that answer, which decides whether a condition needs
        // its context, must change nothing. Wrong behaviour caught: a read that counts, so the next
        // real hit is counted twice and `Equal(4)` misses the stop it should make.
        let mut bp = breakpoint(1, 10, 20, 1);
        bp.hits = 3;
        assert_eq!(
            count_hit(&bp, |_| false, 1, 10),
            Some(CountedHit {
                count: 4,
                passes: true
            })
        );

        bp.hit_condition = Some(HitCondition::Equal(5));
        assert_eq!(
            count_hit(&bp, |_| false, 1, 10),
            Some(CountedHit {
                count: 4,
                passes: false
            })
        );
        assert_eq!(bp.hits, 3);
    }

    #[test]
    fn hit_conditions_use_exact_unsigned_arithmetic() {
        // Specification 13 (runtime debug state): equality matches only the target, at-least
        // includes it, and greater-than excludes it, including at 0 and u64::MAX. Wrong behaviour
        // caught: a signed comparison, which reads u64::MAX as -1, or a greater-than that accepts
        // its own target.
        assert!(HitCondition::Equal(0).is_met(0));
        assert!(HitCondition::AtLeast(0).is_met(0));
        assert!(!HitCondition::GreaterThan(0).is_met(0));
        assert!(HitCondition::GreaterThan(u64::MAX - 1).is_met(u64::MAX));
        assert!(!HitCondition::GreaterThan(u64::MAX).is_met(u64::MAX));
        assert!(!HitCondition::Equal(u64::MAX - 1).is_met(u64::MAX));
    }

    #[test]
    fn cycle_suppression_applies_to_one_location_and_generation_only() {
        // Specification 13: the cycle-suppression identity is (file_id,start,end,generation), and
        // it suppresses only that identity. Wrong behaviour caught: suppressing by span alone,
        // which silences a replaced breakpoint in its new generation, or suppressing across files.
        let mut stopped = breakpoint(1, 10, 20, 7);
        let mut suppression: CycleSuppression<u32> = CycleSuppression::default();
        assert!(observe_hit(&mut stopped, |_| false, 1, 10));
        suppression.record_stop(stopped.id);
        assert!(!observe_hit(
            &mut stopped,
            |id| suppression.suppresses(id),
            1,
            10
        ));

        let mut other_location = breakpoint(1, 30, 40, 7);
        assert!(observe_hit(
            &mut other_location,
            |id| suppression.suppresses(id),
            1,
            30
        ));

        let mut next_generation = breakpoint(1, 10, 20, 8);
        assert!(observe_hit(
            &mut next_generation,
            |id| suppression.suppresses(id),
            1,
            10
        ));

        let mut other_file = breakpoint(2, 10, 20, 7);
        assert!(observe_hit(
            &mut other_file,
            |id| suppression.suppresses(id),
            2,
            10
        ));
    }

    #[test]
    fn a_stop_suppresses_its_breakpoint_for_the_rest_of_that_scan_only() {
        // Specification 13: one breakpoint identity emits at most one stop per scan. Wrong
        // behaviour caught: a suppression that outlives its scan stops the breakpoint only once
        // for the whole run, and a suppression that does not hold within the scan stops it at
        // every start.
        let mut bp = breakpoint(1, 10, 20, 7);
        let mut suppression: CycleSuppression<u32> = CycleSuppression::default();
        assert!(observe_hit(&mut bp, |id| suppression.suppresses(id), 1, 10));
        suppression.record_stop(bp.id);

        assert!(!observe_hit(
            &mut bp,
            |id| suppression.suppresses(id),
            1,
            15
        ));
        suppression.begin_cycle();
        assert!(observe_hit(&mut bp, |id| suppression.suppresses(id), 1, 10));
        // The first scan counted one hit, the suppressed start counted none, and the next scan
        // counted one more.
        assert_eq!(bp.hits, 2);
    }
}
