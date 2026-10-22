//! The fixture that the Tune section tests and the Tune request tests share (specification 35, B4.2, B10.4). Declared under `cfg(test)` by
//! `tune.rs`.

use crate::tuning::candidate::ControlPidCandidate;

/// The `Control.PID` record of an answer or a section: these tests design a `Control.PID` loop, which keeps a record.
pub(super) fn record(candidate: Option<&ControlPidCandidate>) -> &ControlPidCandidate {
    candidate.expect("a Control.PID loop keeps a record")
}
