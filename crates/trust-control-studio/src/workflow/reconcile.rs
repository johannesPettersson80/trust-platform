//! The runtime calls of the reconciliation of an experiment by its identifier (`identificationReconcile.ts`; specification 35,
//! E11.4 to E11.6). A read sends only status requests: nothing is aborted, finished or restored here. The meaning of a read is
//! decided in `trust-control` (`identification::reconcile`).

use trust_control::failure::Failure;
use trust_control::identification::reconcile::{
    experiment_identity, ExperimentIdentity, StatusRead,
};

use super::identify_campaign::CampaignClock;
use crate::runtime_client::recording_coordinator::RecordingSession;

/// Reads one experiment by its identifier alone (`readExperimentIdentity`). `read` makes the status request.
#[must_use]
pub fn read_experiment_identity(
    read: impl FnOnce(&str) -> StatusRead,
    experiment_id: &str,
) -> ExperimentIdentity {
    experiment_identity(experiment_id, read(experiment_id))
}

/// Polls one experiment until it has ended or is absent (`followUntilEnded`), once every `interval_ms` of wall time. Each
/// poll waits first, then asks whether the follow has been released, and only then reads the experiment. Resolves `None`
/// once the caller has released its follow, and sends nothing after that.
#[must_use]
pub fn follow_until_ended(
    mut read: impl FnMut() -> ExperimentIdentity,
    clock: &impl CampaignClock,
    released: impl Fn() -> bool,
    interval_ms: u64,
) -> Option<ExperimentIdentity> {
    loop {
        clock.wait(interval_ms);
        if released() {
            return None;
        }
        let identity = read();
        if matches!(
            identity,
            ExperimentIdentity::Terminal { .. } | ExperimentIdentity::Absent { .. }
        ) {
            return Some(identity);
        }
    }
}

/// Frees a stopped recording's runtime buffer (`releaseRecording`; E4.7, E11.6). Call it only after the recording's samples
/// are kept.
///
/// # Errors
/// The recording cannot be released; the buffer stays pending for the next start or release.
pub fn release_recording<S: RecordingSession>(recording: &mut S) -> Result<(), Failure> {
    recording.release()
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use serde_json::json;
    use trust_control::identification::reconcile::{ExperimentIdentity, StatusRead};

    use super::{follow_until_ended, read_experiment_identity};
    use crate::workflow::identify_campaign::CampaignClock;

    /// A clock that only moves when it is waited on.
    struct ManualClock {
        now: Cell<u64>,
        waits: RefCell<Vec<u64>>,
    }

    impl CampaignClock for ManualClock {
        fn now_ms(&self) -> f64 {
            #[allow(clippy::cast_precision_loss)] // A test clock of a few seconds.
            let now = self.now.get() as f64;
            now
        }

        fn wait(&self, milliseconds: u64) {
            self.now.set(self.now.get() + milliseconds);
            self.waits.borrow_mut().push(milliseconds);
        }
    }

    fn running() -> ExperimentIdentity {
        ExperimentIdentity::Running {
            state: json!({ "status": "running", "restoration": "pending" }),
        }
    }

    fn terminal() -> ExperimentIdentity {
        ExperimentIdentity::Terminal {
            state: json!({ "status": "completed", "restoration": "restored" }),
        }
    }

    #[test]
    fn the_identity_is_read_with_one_status_request_and_its_refusal_keeps_its_code() {
        let mut requests = Vec::new();
        let identity = read_experiment_identity(
            |id| {
                requests.push(id.to_string());
                StatusRead::Refused {
                    code: Some("experiment_not_found".to_string()),
                    text: "No experiment holds this identifier.".to_string(),
                }
            },
            "experiment-1",
        );
        assert_eq!(requests, vec!["experiment-1".to_string()]);
        assert_eq!(
            identity,
            ExperimentIdentity::Absent {
                reason: "No experiment holds this identifier.".to_string()
            }
        );
    }

    #[test]
    fn a_running_experiment_is_followed_until_it_has_ended_then_its_terminal_state_is_returned() {
        let clock = ManualClock {
            now: Cell::new(0),
            waits: RefCell::new(Vec::new()),
        };
        let mut answers = vec![running(), running(), terminal()].into_iter();
        let mut reads = 0;
        let ended = follow_until_ended(
            || {
                reads += 1;
                answers.next().expect("three reads")
            },
            &clock,
            || false,
            500,
        );
        assert_eq!(ended, Some(terminal()));
        assert_eq!(reads, 3, "the experiment is read once per interval");
        assert_eq!(clock.waits.borrow().as_slice(), &[500, 500, 500]);
    }

    #[test]
    fn a_released_follow_sends_nothing_further() {
        let clock = ManualClock {
            now: Cell::new(0),
            waits: RefCell::new(Vec::new()),
        };
        let reads = Cell::new(0);
        let released = Cell::new(false);
        let ended = follow_until_ended(
            || {
                reads.set(reads.get() + 1);
                if reads.get() == 2 {
                    released.set(true);
                }
                running()
            },
            &clock,
            || released.get(),
            500,
        );
        assert_eq!(ended, None, "a released follow resolves with nothing");
        assert_eq!(reads.get(), 2, "the read after the release never happens");
    }

    #[test]
    fn an_absent_identity_ends_the_follow_at_once() {
        let clock = ManualClock {
            now: Cell::new(0),
            waits: RefCell::new(Vec::new()),
        };
        let absent = ExperimentIdentity::Absent {
            reason: "No experiment holds this identifier.".to_string(),
        };
        let ended = follow_until_ended(|| absent.clone(), &clock, || false, 500);
        assert_eq!(ended, Some(absent));
        assert_eq!(clock.waits.borrow().len(), 1);
    }
}
