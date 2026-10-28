//! The status pages of a running recording (runtime protocol annex, E4.6; specification 35, A4.2): the checks of a page
//! against the cursor it answers (`decodeFollow` of `runtimeRecording.ts`), and the cursor the in-order follow carries from
//! one page to the next.
//!
//! A status page carries no loop revision, so the request that names the recording checks its identity (`recording_status`).

use crate::failure::{Cause, Failure, NextAction};
use crate::recording::samples::{
    aligned, check_bool_values, invalid_samples, samples_increase, SamplePosition,
};
use crate::recording::RecordingStatus;
use crate::signals::SignalBinding;

/// The most samples one status page holds (E4.6).
pub const MAXIMUM_PAGE_SAMPLES: usize = 2_000;

/// Where the in-order follow of a recording continues: the index of the next sample, and the last sample read, which the
/// first sample of the next page must follow (`followCursor` of `runtimeRecording.ts`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FollowCursor {
    /// The index the next request starts from.
    pub next_sample: u64,
    /// The last sample read; none before the first sample.
    pub last: Option<SamplePosition>,
}

impl FollowCursor {
    /// The cursor after `page`: the next request starts at the page's `next_sample`, and the last sample read is the page's
    /// last one. A page without samples keeps the last sample read before it.
    #[must_use]
    pub fn advance(self, page: &RecordingStatus) -> Self {
        Self {
            next_sample: page.next_sample,
            last: page
                .samples
                .last()
                .and_then(SamplePosition::of)
                .or(self.last),
        }
    }
}

/// Checks a status page against the cursor it answers (E4.6): the page holds at most [`MAXIMUM_PAGE_SAMPLES`] samples, its
/// sample count is not below the index it started from, and its next index is the index it started from plus the samples it
/// holds, and at most its sample count. Its samples are well formed, step forward from the last sample read, and hold BOOL
/// values of 0 or 1.
///
/// # Errors
/// The page's counts do not agree, or a sample is malformed, out of order or a BOOL value is neither 0 nor 1.
pub fn check_status_page(
    page: &RecordingStatus,
    cursor: &FollowCursor,
    bindings: &[SignalBinding],
) -> Result<(), Failure> {
    let from = cursor.next_sample;
    let read = u64::try_from(page.samples.len()).unwrap_or(u64::MAX);
    let counts_agree = page.sample_count >= from
        && page.samples.len() <= MAXIMUM_PAGE_SAMPLES
        && from.checked_add(read) == Some(page.next_sample)
        && page.next_sample <= page.sample_count;
    if !counts_agree {
        return Err(Failure::new(
            "The runtime recording status is invalid.",
            Cause::Runtime,
            NextAction::None,
        ));
    }
    if !aligned(&page.samples, bindings.len()) || !samples_increase(&page.samples, cursor.last) {
        return Err(invalid_samples());
    }
    check_bool_values(bindings, &page.samples)
}

#[cfg(test)]
mod tests {
    use crate::recording::fixtures::{binding, sample};
    use crate::recording::{RecordedSample, RecordingState, RecordingStatus};
    use crate::signals::{Quality, SignalBinding};

    use super::{check_status_page, FollowCursor, MAXIMUM_PAGE_SAMPLES};

    fn bindings() -> Vec<SignalBinding> {
        vec![
            binding("processVariable", "%IL0", "LREAL"),
            binding("valid", "%MX16.0", "BOOL"),
        ]
    }

    fn scan(index: u64) -> RecordedSample {
        sample(
            index,
            Some(i64::try_from(index * 10_000_000).unwrap_or(i64::MAX)),
            &[Some(2.5), Some(1.0)],
            &[Quality::Good, Quality::Good],
        )
    }

    fn page(from: u64, samples: Vec<RecordedSample>, sample_count: u64) -> RecordingStatus {
        let next_sample = from + u64::try_from(samples.len()).unwrap_or(0);
        RecordingStatus {
            recording_id: "recording-1".into(),
            state: RecordingState::Recording,
            elapsed_ms: 10,
            sample_count,
            complete: false,
            reason: None,
            samples,
            next_sample,
        }
    }

    #[test]
    fn a_page_is_checked_against_the_index_it_starts_from() {
        let cursor = FollowCursor::default();
        assert!(
            check_status_page(&page(0, vec![scan(1), scan(2)], 2), &cursor, &bindings()).is_ok()
        );
        let mut wrong_next = page(0, vec![scan(1), scan(2)], 2);
        wrong_next.next_sample = 1;
        assert!(
            check_status_page(&wrong_next, &cursor, &bindings()).is_err(),
            "next_sample is the index from which the samples count"
        );
        let mut beyond = page(0, vec![scan(1), scan(2)], 1);
        beyond.next_sample = 2;
        assert!(
            check_status_page(&beyond, &cursor, &bindings()).is_err(),
            "next_sample is at most sample_count"
        );
        let from_three = FollowCursor {
            next_sample: 3,
            last: None,
        };
        assert!(
            check_status_page(&page(3, vec![], 2), &from_three, &bindings()).is_err(),
            "the recording holds fewer samples than the page starts from"
        );
    }

    #[test]
    fn a_page_holds_at_most_two_thousand_samples() {
        let samples: Vec<RecordedSample> = (1..=u64::try_from(MAXIMUM_PAGE_SAMPLES).unwrap_or(0)
            + 1)
            .map(scan)
            .collect();
        let count = u64::try_from(samples.len()).unwrap_or(0);
        assert!(
            check_status_page(
                &page(0, samples, count),
                &FollowCursor::default(),
                &bindings()
            )
            .is_err(),
            "a page of 2,001 samples is refused (E4.6)"
        );
    }

    #[test]
    fn samples_step_forward_across_pages_and_a_bool_is_zero_or_one() {
        let first = page(0, vec![scan(1), scan(2)], 4);
        let cursor = FollowCursor::default().advance(&first);
        assert!(check_status_page(&page(2, vec![scan(3)], 4), &cursor, &bindings()).is_ok());
        assert!(
            check_status_page(&page(2, vec![scan(2)], 4), &cursor, &bindings()).is_err(),
            "the next page does not repeat the last scan of the page before"
        );
        let mut bad_bool = scan(3);
        bad_bool.values[1] = Some(0.5);
        assert!(check_status_page(&page(2, vec![bad_bool], 3), &cursor, &bindings()).is_err());
    }

    #[test]
    fn the_cursor_keeps_the_last_sample_read_across_an_empty_page() {
        let first = page(0, vec![scan(1), scan(2)], 2);
        let cursor = FollowCursor::default().advance(&first);
        let empty = page(2, vec![], 2);
        let next = cursor.advance(&empty);
        assert_eq!(next.next_sample, 2);
        assert_eq!(
            next.last.map(|last| last.scan),
            Some(2),
            "an empty page keeps the last sample read"
        );
    }
}
