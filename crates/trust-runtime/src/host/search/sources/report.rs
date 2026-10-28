//! What a protocol search reports, and when it stops (specification 11, "comm.discover" and
//! "Search"). A protocol search hands each candidate to its sink as soon as it finds it, and
//! starts no probe after its deadline. `comm.discover` collects the candidates into its answer
//! and gives no deadline; a search keeps each candidate as a result of its device until the
//! search ends.

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

/// One thing a protocol search found, as `comm.discover` lists it.
#[derive(Debug, Serialize)]
pub(crate) struct DiscoverCandidate {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) source: &'static str,
    pub(super) confidence: &'static str,
    pub(super) params: Value,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) warnings: Vec<String>,
}

/// Where a protocol search reports.
pub(crate) trait CandidateSink {
    /// Takes one candidate, as soon as the search finds it.
    fn found(&mut self, candidate: DiscoverCandidate);
    /// Takes one warning about what the search could not search.
    fn warn(&mut self, warning: String);
}

/// When a protocol search stops.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Deadline(Option<Instant>);

impl Deadline {
    /// No deadline: each probe ends by its own timeout, as for a `comm.discover` request.
    pub(crate) const NONE: Self = Self(None);

    /// The deadline at `instant`.
    pub(super) fn at(instant: Instant) -> Self {
        Self(Some(instant))
    }

    /// Whether the deadline has passed; a protocol search starts nothing after it.
    pub(super) fn passed(self) -> bool {
        self.0.is_some_and(|at| Instant::now() >= at)
    }

    /// How long one probe may wait: `timeout`, cut to the whole milliseconds left before the
    /// deadline, and at least one millisecond.
    pub(super) fn bound(self, timeout: Duration) -> Duration {
        match self.0 {
            None => timeout,
            Some(at) => {
                let left = at.saturating_duration_since(Instant::now());
                Duration::from_millis(millis(timeout.min(left)).max(1))
            }
        }
    }
}

/// `value` with every character except ASCII letters, digits, `-`, `_` and `.` replaced by `_`.
pub(super) fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// A duration in whole milliseconds.
pub(super) fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
