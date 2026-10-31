//! Tests of the wait for a loop found in Auto to rest (`identify_rest`; specification 35, B3.4 step 1, A3.2, A3.4) on scripted
//! transfer readings: process time from the runtime's clock, the reading at rest returned, the cancel, a reading without a clock or a
//! value, a runtime that stops answering, and the progress shown once. The decisions themselves are tested in trust-control
//! (`campaign/rest_tests.rs`). The loop is the apply tests' PID loop: `Main.PV` in m (0 to 10 m) and `Main.MV` in V; with a 200 s
//! maximum and readings 0.1 s of process time apart, the baseline window is 10 s. A PID group (C7) is the fixture
//! `control-studio-pid-group`, whose two members' process values and outputs all run from 0 to 100 on a 100 ms task: the same window.

use std::cell::{Cell, RefCell};

use trust_control::commissioning::commissioning_result::{TransferState, TransferStatus};
use trust_control::commissioning::transfer::TransferValue;
use trust_control::failure::Failure;
use trust_control::identification::mode_transfer::mode_binding;
use trust_control::limits::CONNECTION_LOSS_TIMEOUT_MS;
use trust_control::loop_definition::LoopDefinition;

use super::RestWait;
use crate::workflow::apply::test_support::{pid_loop, Report};
use crate::workflow::identify_campaign::CampaignClock;

/// A wall clock that each pause moves on by the pause.
struct TestClock {
    now: Cell<f64>,
}

impl CampaignClock for TestClock {
    fn now_ms(&self) -> f64 {
        self.now.get()
    }

    fn wait(&self, milliseconds: u64) {
        #[allow(clippy::cast_precision_loss)] // A poll is 100 ms; the test clock never nears 2⁵³.
        self.now.set(self.now.get() + milliseconds as f64);
    }
}

fn clock() -> TestClock {
    TestClock {
        now: Cell::new(0.0),
    }
}

/// One transfer reading of `Main.PV` and `Main.MV`, taken at `runtime_time_ns` of the runtime's clock (E6.3); `None` leaves a value
/// out.
fn reading(pv: Option<f64>, mv: Option<f64>, runtime_time_ns: Option<u64>) -> TransferStatus {
    let values = [("Main.PV", pv), ("Main.MV", mv)]
        .into_iter()
        .filter_map(|(symbol, value)| Some((symbol.to_owned(), TransferValue::Number(value?))))
        .collect();
    TransferStatus {
        program_revision: "program-1".into(),
        binding_revision: "transfer-1".into(),
        values,
        runtime_time_ns,
        status: TransferState::Idle,
        reason: None,
        supported_tracking: None,
        transaction: None,
    }
}

/// The loop file of the two-member PID group: `P1.Controller` on `P1.ProcessValue` and `P1.ManipulatedValue`, `P1.Secondary` on
/// `P1.SecondaryProcessValue` and `P1.SecondaryManipulatedValue`, sharing the mode `P1.AutoMode`.
const PID_GROUP: &str = include_str!(
    "../../../../editors/vscode/src/test/fixtures/control-studio-pid-group/control-loop.json"
);

/// One transfer reading of the group's process values and outputs, in the order the rest watches them, taken at `runtime_time_ns`.
fn group_reading(values: [f64; 4], runtime_time_ns: u64) -> TransferStatus {
    let symbols = [
        "P1.ProcessValue",
        "P1.SecondaryProcessValue",
        "P1.ManipulatedValue",
        "P1.SecondaryManipulatedValue",
    ];
    TransferStatus {
        values: symbols
            .into_iter()
            .zip(values)
            .map(|(symbol, value)| (symbol.to_owned(), TransferValue::Number(value)))
            .collect(),
        ..reading(None, None, Some(runtime_time_ns))
    }
}

/// Waits on readings that `script` gives for each read (counted from 0), with a 200 s maximum and no setpoint step offered; returns
/// the result and the number of readings taken.
fn wait_on(
    report: &Report,
    clock: &TestClock,
    script: impl Fn(u64) -> TransferStatus,
) -> (Result<TransferStatus, Failure>, u64) {
    wait_on_loop(&pid_loop(), report, clock, script)
}

/// [`wait_on`] for `loop_definition`, before the transfer of its mode binding.
fn wait_on_loop(
    loop_definition: &LoopDefinition,
    report: &Report,
    clock: &TestClock,
    script: impl Fn(u64) -> TransferStatus,
) -> (Result<TransferStatus, Failure>, u64) {
    let binding = mode_binding(loop_definition).expect("the loop's mode binding");
    let mut rest = RestWait::new(loop_definition, 200_000, None, false, report, clock);
    let reads = RefCell::new(0_u64);
    let result = rest.wait(&binding, &mut || {
        let index = *reads.borrow();
        *reads.borrow_mut() += 1;
        Ok(script(index))
    });
    (result, reads.into_inner())
}

/// The runtime's clock after `index` readings that are `step_ms` of process time apart, from 1 s.
fn runtime_ns(index: u64, step_ms: u64) -> u64 {
    1_000_000_000 + index * step_ms * 1_000_000
}

#[test]
fn a_loop_at_rest_is_returned_once_one_window_of_process_time_is_full() {
    // Readings 0.1 s apart at a 0.1 s poll: the reading at 10 s, the 101st, ends the wait and is the one returned for the transfer.
    let report = Report::default();
    let clock = clock();
    let (result, reads) = wait_on(&report, &clock, |index| {
        reading(Some(5.0), Some(2.0), Some(runtime_ns(index, 100)))
    });
    let rested = result.expect("a loop at rest rests");
    assert_eq!(reads, 101);
    assert_eq!(rested.runtime_time_ns, Some(runtime_ns(100, 100)));
}

#[test]
fn the_wait_is_measured_in_the_runtimes_process_time_not_the_wall_clock() {
    // The runtime advances 0.05 s of process time for each 0.1 s poll, as a runner that reaches half the requested speed: the 10 s
    // window is full at the 201st reading, after 20 s of wall time. Catches a wait measured on the wall clock (macOS's runner ran
    // lesson 30 at 2.4 times real time instead of ten, 30 October 2026).
    let report = Report::default();
    let clock = clock();
    let (result, reads) = wait_on(&report, &clock, |index| {
        reading(Some(5.0), Some(2.0), Some(runtime_ns(index, 50)))
    });
    assert!(result.is_ok());
    assert_eq!(reads, 201);
}

#[test]
fn a_cancel_ends_the_wait_before_any_reading() {
    let report = Report::default();
    report.cancel();
    let clock = clock();
    let (result, reads) = wait_on(&report, &clock, |index| {
        reading(Some(5.0), Some(2.0), Some(runtime_ns(index, 100)))
    });
    assert_eq!(
        result.expect_err("cancelled").message,
        "Identification cancelled."
    );
    assert_eq!(reads, 0);
}

#[test]
fn a_reading_without_the_runtimes_clock_or_a_value_ends_the_wait() {
    let clock = clock();
    for (script, text) in [
        (
            reading(Some(5.0), Some(2.0), None),
            "The runtime does not report its clock in the transfer status.",
        ),
        (
            reading(None, Some(2.0), Some(runtime_ns(0, 100))),
            "The runtime did not return a finite Main.PV value.",
        ),
        (
            reading(Some(5.0), None, Some(runtime_ns(0, 100))),
            "The runtime did not return a finite Main.MV value.",
        ),
    ] {
        let report = Report::default();
        let (result, reads) = wait_on(&report, &clock, |_| script.clone());
        assert_eq!(result.expect_err("refused").message, text);
        assert_eq!(reads, 1);
    }
}

#[test]
fn a_runtime_whose_clock_stops_ends_the_wait_after_the_connection_loss_timeout() {
    // Every reading carries the same runtime time, so no newer value arrives: the wait ends once more than the 5 s timeout of wall
    // time has passed since the first reading (A3.4), at the 52nd reading, 5.1 s after it.
    assert_eq!(CONNECTION_LOSS_TIMEOUT_MS, 5_000);
    let report = Report::default();
    let clock = clock();
    let (result, reads) = wait_on(&report, &clock, |_| {
        reading(Some(5.0), Some(2.0), Some(runtime_ns(0, 100)))
    });
    assert_eq!(
        result.expect_err("stopped").message,
        "The runtime stopped sending values."
    );
    assert_eq!(reads, 52);
}

#[test]
fn a_moving_loop_shows_its_progress_once_until_it_changes() {
    // The measurement rises 0.01 m/s for 30 s of process time and then rests: the progress names it, and each text is shown once.
    let report = Report::default();
    let clock = clock();
    let (result, _) = wait_on(&report, &clock, |index| {
        #[allow(clippy::cast_precision_loss)] // A few hundred readings.
        let seconds = index as f64 / 10.0;
        reading(
            Some(4.0 + 0.01 * seconds.min(30.0)),
            Some(2.0),
            Some(runtime_ns(index, 100)),
        )
    });
    let rested = result.expect("the loop rests once the measurement stops");
    let lines = report.lines();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Main.PV changing 0.6 m/min")),
        "{lines:?}"
    );
    assert!(lines.windows(2).all(|pair| pair[0] != pair[1]), "{lines:?}");
    let pv = rested
        .values
        .get("Main.PV")
        .copied()
        .and_then(TransferValue::as_number);
    assert_eq!(pv, Some(4.3));
}

#[test]
fn a_pid_group_rests_only_once_its_second_members_output_rests() {
    // Codex's review of #147 (31 October 2026): the wait read the loop's own pair alone, while the group's transfer holds both
    // outputs. The second member's output falls 0.02 % per second until 30 s of process time, 0.2 % across any 10 s window, while
    // every other signal is flat: the progress names it, and the wait cannot end before 30 s and ends by 40 s, when a whole window
    // after the stop is flat.
    let loop_definition = LoopDefinition::decode(PID_GROUP).expect("the group's loop file");
    let report = Report::default();
    let clock = clock();
    let (result, _) = wait_on_loop(&loop_definition, &report, &clock, |index| {
        #[allow(clippy::cast_precision_loss)] // A few hundred readings.
        let seconds = index as f64 / 10.0;
        group_reading(
            [40.0, 60.0, 30.0, 50.0 - 0.02 * seconds.min(30.0)],
            runtime_ns(index, 100),
        )
    });
    let rested = result.expect("the group rests once its second output stops");
    let lines = report.lines();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("P1.SecondaryManipulatedValue changing 1.2 %/min")),
        "{lines:?}"
    );
    let at = rested.runtime_time_ns.expect("the reading's clock");
    assert!(
        at > runtime_ns(300, 100) && at <= runtime_ns(400, 100),
        "rested at {at} ns"
    );
}

#[test]
fn a_group_reading_without_the_second_members_output_ends_the_wait() {
    // Catches a wait that reads the loop's own pair alone: the second member's output is one of the values it needs.
    let loop_definition = LoopDefinition::decode(PID_GROUP).expect("the group's loop file");
    let mut status = group_reading([40.0, 60.0, 30.0, 50.0], runtime_ns(0, 100));
    status.values.remove("P1.SecondaryManipulatedValue");
    let report = Report::default();
    let clock = clock();
    let (result, reads) = wait_on_loop(&loop_definition, &report, &clock, |_| status.clone());
    assert_eq!(
        result.expect_err("refused").message,
        "The runtime did not return a finite P1.SecondaryManipulatedValue value."
    );
    assert_eq!(reads, 1);
}
