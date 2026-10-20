//! What an online change keeps and what it refuses (specification 11 §6.7.1; specification 34 §11.3,
//! STATE-08 and UPDATE-01 to UPDATE-07): the lines and their output levels, logical time and the scan
//! counter, the five-second bound, the retained values, the installed program, the refusals before the
//! commit, a faulted program and a stop. The harness is the one of the parent module.

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use serde_json::json;
use trust_runtime_core::deployment::SafeStateEntry;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{self, Value as Retained};
use trust_runtime_core::watchdog::RetainMode;

use super::{
    address, entries, no_replies, plant, record, reload, reloaded, request, texts, Rig, BOOT,
};
use crate::live_values::var_requests::VariableQueuePort;
use crate::runner::check_retained_values;
use crate::runner::hold_contract_tests::on_led_assignment;
use crate::runner::program::Program;
use crate::tests::program_store::{committed_image, CheckpointProbe, FlashProbe, PROGRAM};
use crate::tests::runner_ports::{ms, IoStep, TestPlatform, GPIO};
use crate::transfer::TransferSession;

/// A checkpoint whose newest snapshot holds `value` under `name`.
fn checkpoint_of(name: &str, value: Retained) -> CheckpointProbe {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert(name, value);
    CheckpointProbe::with_checkpoint(&snapshot)
}

/// The steps of one scan of the board probe at the process I/O: the inputs are read, and the output
/// image `led` is written.
fn scan_steps(led: u8) -> Vec<IoStep> {
    vec![IoStep::ReadInputs, IoStep::WriteOutputs(vec![led])]
}

#[test]
fn an_online_change_keeps_the_lines_and_their_output_levels() {
    // Specification 34 §11.3: the change keeps the running program's lines claimed, with the levels
    // their outputs hold: no line is claimed or released, and no output is written between the commit
    // and the candidate's first output commit. The probe's `led` follows `button`, which is high, from
    // its first activation at 10 ms: its scans at 0, 10 and 20 ms read the input and write the output
    // image 0, 1 and 1. The change at 25 ms is committed at the boundary at 30 ms, and what reaches the
    // machine's lines in that pass is exactly the candidate's first scan: the inputs read and one
    // output image written, 1, because the candidate's task continues the running program's schedule
    // (decision OC-2): it last ran at 20 ms and is due at 30 ms, so it computes `led` from `button`
    // before the outputs are written; no safe-state write and no other output. At 40 ms the candidate
    // drives 1 again. The lines have the same holders before and after, and were claimed once. Wrong
    // behaviour caught: a candidate that claims the lines again (a second holder); a running program
    // that releases them before the candidate has them; an output written at the commit, or the safe
    // state applied for the program that is dropped; a schedule that starts again at the commit, so
    // that the first scan runs no task and the output drops to its initial value 0 for a cycle.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.ports.io.state.borrow_mut().inputs = vec![1];
    for at in [0, 10, 20] {
        rig.pass_at(&mut runner, at);
    }
    let mut written = scan_steps(0);
    written.extend(scan_steps(1));
    written.extend(scan_steps(1));
    assert_eq!(
        rig.ports.io.take_steps(),
        written,
        "each scan reads the inputs and writes the output image"
    );
    let holders = Rc::strong_count(&rig.ports.io.state);

    assert_eq!(rig.call_at(&mut runner, 25, &reload(3, GPIO)), no_replies());
    assert_eq!(
        rig.ports.io.take_steps(),
        Vec::<IoStep>::new(),
        "the request writes nothing to the lines"
    );
    rig.pass_at(&mut runner, 30);
    assert_eq!(rig.replies(), [reloaded(3, 1)]);
    assert_eq!(
        rig.ports.io.take_steps(),
        scan_steps(1),
        "the lines see the candidate's first scan, which runs the due task, and nothing before it"
    );
    assert_eq!(Rc::strong_count(&rig.ports.io.state), holders);
    rig.pass_at(&mut runner, 40);
    assert_eq!(
        rig.ports.io.take_steps(),
        scan_steps(1),
        "the candidate drives the lines on its schedule"
    );
    assert_eq!(rig.claims.get(), 1, "no line was claimed again");
    assert!(
        !entries(&rig.log)
            .iter()
            .any(|entry| entry.starts_with("safe outputs")),
        "the safe state is not applied for a change"
    );
}

#[test]
fn an_online_change_preserves_logical_time_and_restarts_the_scan_counter() {
    // Specification 11 §6.7.1 and specification 34 §11.3 (decision OC-2): logical time is preserved,
    // the task schedule continues and the scan counter restarts. The probe runs five scans, 0 to 40
    // ms; the change at 45 ms is committed at the boundary at 50 ms. The candidate keeps the running
    // program's task timing (no timing starts at the change) and scans at 50 ms (not at 0), the
    // program's time is 50 ms and its scan counter is 1 after the first scan, 2 after the next. The
    // events of the program stay and the cycle events of the candidate count from 0 again: the event
    // store holds the old program's cycle 4 at 40 ms and the candidate's cycle 0 at 50 ms. Wrong
    // behaviour caught: a change that starts the program's time at 0 as a restart does, or its task
    // timing at the change; a scan counter that goes on; an event store that is emptied.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    for at in [0, 10, 20, 30, 40] {
        rig.pass_at(&mut runner, at);
    }
    assert_eq!(runner.cycle_count, 5);
    assert_eq!(runner.program_time, value::Duration::from_millis(40));

    assert_eq!(rig.call_at(&mut runner, 45, &reload(4, GPIO)), no_replies());
    rig.pass_at(&mut runner, 50);
    assert_eq!(rig.replies(), [reloaded(4, 1)]);
    let log = entries(&rig.log);
    assert!(
        log.ends_with(&texts(&["commit", "drop running", "scan change 1 50",])),
        "{log:?}"
    );
    assert_eq!(runner.program_time, value::Duration::from_millis(50));
    assert_eq!(runner.cycle_count, 1);

    rig.pass_at(&mut runner, 60);
    assert!(entries(&rig.log).ends_with(&texts(&["scan change 1 60"])));
    assert_eq!(runner.cycle_count, 2);

    let events = rig.call_at(
        &mut runner,
        61,
        &json!({"id": 5, "type": "events.tail", "params": {"limit": 100}}),
    );
    let events = events[0]["result"]["events"]
        .as_array()
        .expect("the events");
    for expected in [
        json!({"type": "cycle_end", "cycle": 4, "time_ns": 40_000_000}),
        json!({"type": "cycle_end", "cycle": 0, "time_ns": 50_000_000}),
    ] {
        assert!(events.contains(&expected), "{expected} in {events:?}");
    }
}

#[test]
fn a_reload_the_program_does_not_take_within_five_seconds_is_withdrawn_with_reload_timeout() {
    // Specification 11 §6.7.1: a reload waits up to five seconds for the resource to take it; if it has
    // not started applying it by then, the request is withdrawn and never applied, and the reply is
    // `reload timeout`. The program's cycle is 10 s here, so its next boundary is far off. The change
    // arrives at 1,000 ms: nothing is written at 5,999 ms, and at 6,000 ms the reply is `reload
    // timeout` with no error code. The loop waited for exactly that moment (its deadline is the
    // earlier of the boundary at 10,000 ms, the expiry and the 60 s feed interval). The change is never
    // applied: at the boundary at 10,000 ms the program scans on, and nothing was prepared. The loop
    // reads requests again after the timeout. Wrong behaviour caught: a bound of a different length, or
    // one that is off by a pass; a withdrawn change that is applied at the boundary; a wait that does
    // not wake for the expiry; a loop that stays deaf after the timeout.
    let rig = Rig::new();
    let mut config = record(RetainMode::None, &[], &[]);
    config.cycle_interval_nanos = 10_000_000_000;
    let platform = TestPlatform {
        feed_interval: core::time::Duration::from_secs(60),
        ..TestPlatform::default()
    };
    let mut runner = rig.runner_from(PROGRAM, config, CheckpointProbe::erased(), None, platform);
    rig.pass_at(&mut runner, 0);
    let _ = rig.ports.wait.take_deadlines();

    assert_eq!(
        rig.call_at(&mut runner, 1_000, &reload(6, PROGRAM)),
        no_replies()
    );
    assert_eq!(
        rig.ports.wait.take_deadlines(),
        [ms(6_000)],
        "the loop waits until the change would be withdrawn"
    );
    rig.pass_at(&mut runner, 5_999);
    assert_eq!(
        rig.replies(),
        no_replies(),
        "within the five seconds nothing is written"
    );
    rig.pass_at(&mut runner, 6_000);
    assert_eq!(
        rig.replies(),
        [json!({"id": 6, "ok": false, "error": "reload timeout"})],
        "withdrawn at five seconds, as a computer's reload is"
    );
    let status = rig.call_at(&mut runner, 6_001, &request(7, "status"));
    assert_eq!(
        status[0]["ok"],
        json!(true),
        "the loop serves requests again"
    );

    rig.pass_at(&mut runner, 10_000);
    let log = entries(&rig.log);
    assert!(!log.contains(&String::from("prepare")), "{log:?}");
    assert!(log.ends_with(&texts(&["scan running 10000"])), "{log:?}");
}

#[test]
fn an_online_change_keeps_the_values_and_applies_no_checkpoint() {
    // Specification 11 §6.7.1 and specification 34 §11.3 (decision OC-1): the candidate takes the
    // values the running program holds at the boundary, retained or not, as a computer's does, and no
    // saved snapshot is applied over them: a checkpoint holds older values, and stays the source of a
    // restart after a stop or a power loss. The A4 program runs 0 to 80 ms, with `activations` 3 and
    // `result` 3; the change is committed at 90 ms, before the task is due again at 100 ms.
    //   a record without a retain store: the running program's values carry over, `activations` 3 and
    //     the non-retained `result` 3;
    //   a record that keeps retained values, whose newest checkpoint holds 2: the running program's 3
    //     stays (before decision OC-1 the checkpoint's 2 replaced it);
    //   a checkpoint that names another variable only: the running program's 3 stays.
    // Wrong behaviour caught: values reset to their declared initial values (a non-retained `result`
    // 0, as before checklist C1-24); a checkpoint that replaces the values the program holds.
    let cases = [
        (
            "without a store",
            record(RetainMode::None, &[], &[]),
            CheckpointProbe::erased(),
            Retained::DInt(3),
        ),
        (
            "with a checkpoint that names the variable",
            record(RetainMode::File, &[], &[]),
            checkpoint_of("@program/Plant/activations", Retained::DInt(2)),
            Retained::DInt(3),
        ),
        (
            "with a checkpoint that names another variable",
            record(RetainMode::File, &[], &[]),
            checkpoint_of("@program/Plant/other", Retained::DInt(9)),
            Retained::DInt(3),
        ),
    ];
    for (kind, config, checkpoints, expected) in cases {
        let rig = Rig::new();
        let mut runner = rig.runner(PROGRAM, config, checkpoints);
        for at in [0, 10, 20, 30, 40, 50, 60, 70, 80] {
            rig.pass_at(&mut runner, at);
        }
        assert_eq!(plant(&runner, "activations"), Retained::DInt(3), "{kind}");
        assert_eq!(plant(&runner, "result"), Retained::Int(3), "{kind}");

        assert_eq!(
            rig.call_at(&mut runner, 85, &reload(1, PROGRAM)),
            no_replies(),
            "{kind}"
        );
        rig.pass_at(&mut runner, 90);
        assert_eq!(rig.replies(), [reloaded(1, 1)], "{kind}");
        assert!(
            entries(&rig.log).contains(&String::from("checkpoint reads 0")),
            "{kind}: the change reads no checkpoint"
        );
        assert_eq!(plant(&runner, "activations"), expected, "{kind}");
        assert_eq!(plant(&runner, "result"), Retained::Int(3), "{kind}");
    }
}

#[test]
fn after_an_online_change_a_restart_runs_the_installed_program() {
    // Specification 34 §11.3 and STATE-08: an online change is memory only. The installed program is
    // the A4 container (sequence 1) that the loop's program came from. Without a change, a stop and a
    // `restart` restart the program in place. After a change, the program is not the installed one, so
    // a stop and a `restart` load the installed container (sequence 1, cold), and the loop's first scan
    // after it is the installed program's at time 0; the changed program is never restarted. Wrong
    // behaviour caught: a changed program that survives a stop and a restart; an in-place restart of
    // the program the change gave; a `restart` without a change that reloads the container.
    let slot = |image: &[u8]| {
        let mut bytes = vec![0xff_u8; crate::tests::program_store::SLOT_BYTES as usize];
        bytes[..image.len()].copy_from_slice(image);
        bytes
    };
    for changed in [false, true] {
        let rig = Rig::new();
        let flash = FlashProbe::from_slots([
            slot(&committed_image(1, RetainMode::None)),
            vec![0xff_u8; crate::tests::program_store::SLOT_BYTES as usize],
        ]);
        let mut checkpoints = CheckpointProbe::erased();
        let mut runner = rig
            .runner_from(
                PROGRAM,
                record(RetainMode::None, &[], &[]),
                checkpoints.clone(),
                Some(1),
                TestPlatform::default(),
            )
            .with_transfers(TransferSession::new(BOOT, 65_536, 8_192))
            .with_program_store(flash.store(), move |admitted| {
                check_retained_values(&mut checkpoints, admitted)
            });
        rig.pass_at(&mut runner, 0);
        rig.pass_at(&mut runner, 10);
        if changed {
            assert_eq!(
                rig.call_at(&mut runner, 12, &reload(1, PROGRAM)),
                no_replies()
            );
        }
        // The boundary at 20 ms is taken before the `shutdown` arrives at 22 ms, so that the request
        // is read in its own pass and not behind a boundary that is due.
        rig.pass_at(&mut runner, 20);
        if changed {
            assert_eq!(rig.replies(), [reloaded(1, 1)], "the change is applied");
        }
        let stopping = rig.call_at(&mut runner, 22, &request(2, "shutdown"));
        assert_eq!(stopping[0]["result"], json!({"status": "stopping"}));
        rig.pass_at(&mut runner, 30);
        assert_eq!(runner.resource_state(), ResourceState::Stopped, "{changed}");
        let _ = rig.log.borrow_mut().drain(..);

        let restarted = rig.call_at(
            &mut runner,
            35,
            &json!({"id": 3, "type": "restart", "params": {"mode": "cold"}}),
        );
        assert_eq!(restarted[0]["result"], json!({"status": "restart queued"}));
        let log = entries(&rig.log);
        if changed {
            assert_eq!(
                log,
                texts(&[
                    "drop change 1",
                    "load 1 Cold",
                    "timing installed 1 0",
                    "scan installed 1 0",
                ]),
                "the installed container is loaded in place of the changed program"
            );
        } else {
            assert_eq!(
                log,
                texts(&["restart running Cold", "timing running 0", "scan running 0"]),
                "without a change the program restarts in place"
            );
        }
        assert_eq!(runner.resource_state(), ResourceState::Running, "{changed}");
    }
}

#[test]
fn a_binding_to_an_address_without_a_line_is_refused_before_the_commit() {
    // Specification 34 §11.3: the candidate's bindings are checked against the lines of the running
    // program's record before the commit, as a computer's `validate_replacement_policy` refuses a
    // replacement that cannot take the running configuration. The running A4 program's record gives the
    // input line `%IX0.0` and no output line. The board probe binds `%IX0.0` and `%QX0.0`, so the change
    // is refused with the output's address, nothing is committed and the program runs on; with the
    // output line in the record the same change is applied. Wrong behaviour caught: a binding that is
    // wired to nothing and runs; a refusal after the commit; a check that looks at inputs only.
    let rig = Rig::new();
    let mut runner = rig.runner(
        PROGRAM,
        record(RetainMode::None, &["%IX0.0"], &[]),
        CheckpointProbe::erased(),
    );
    for at in [0, 10] {
        rig.pass_at(&mut runner, at);
    }
    assert_eq!(rig.call_at(&mut runner, 15, &reload(1, GPIO)), no_replies());
    rig.pass_at(&mut runner, 20);
    let refused = rig.replies();
    assert_eq!(refused[0]["ok"], json!(false));
    assert_eq!(
        refused[0]["error"],
        json!("the candidate binds %QX0.0, which the running program's record gives no line")
    );
    let log = entries(&rig.log);
    assert!(log.contains(&String::from("prepare")));
    assert!(
        !log.contains(&String::from("commit")),
        "refused before the commit: {log:?}"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);

    let rig = Rig::new();
    let mut runner = rig.runner(
        PROGRAM,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.pass_at(&mut runner, 0);
    assert_eq!(rig.call_at(&mut runner, 5, &reload(1, GPIO)), no_replies());
    rig.pass_at(&mut runner, 10);
    assert_eq!(
        rig.replies(),
        [reloaded(1, 1)],
        "wired, the change is applied"
    );
}

#[test]
fn a_safe_output_the_candidate_cannot_take_is_refused_before_the_commit() {
    // Specification 34 §11.3: as a computer's `validate_replacement_policy` refuses a replacement whose
    // process image cannot take a `[[io.safe_state]]` entry of the running configuration, the change is
    // refused, because a stop or a fault would write that value into the image. The record keeps `%QX3.0`
    // safe at TRUE, a byte beyond the one-byte output image of the board probe, so the change to the
    // board probe is refused after its candidate was built: the candidate is dropped, nothing is
    // committed, and the running program is as it was. Wrong behaviour caught: a candidate that cannot
    // apply the safe state, committed.
    let rig = Rig::new();
    let mut config = record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]);
    config.safe_state = vec![SafeStateEntry {
        address: address("%QX3.0"),
        value: Retained::Bool(true),
    }];
    let mut runner = rig.runner(GPIO, config, CheckpointProbe::erased());
    rig.pass_at(&mut runner, 0);
    assert_eq!(rig.call_at(&mut runner, 5, &reload(1, GPIO)), no_replies());
    rig.pass_at(&mut runner, 10);
    let refused = rig.replies();
    assert_eq!(refused[0]["ok"], json!(false));
    let log = entries(&rig.log);
    assert!(log.contains(&String::from("prepare")));
    assert!(!log.contains(&String::from("commit")), "{log:?}");
    assert!(
        log.contains(&String::from("drop change 1")),
        "the candidate that was built is dropped: {log:?}"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn an_online_change_recovers_a_faulted_program_as_it_does_on_a_computer() {
    // Specification 11 §6.6: a halting fault leaves the resource waiting in the faulted state for a
    // recovery, a restart or an online change, and a change that succeeds returns it to Running with its
    // fault cleared. The probe's scan at 20 ms fails (division by zero), so the resource is Faulted. A
    // change arrives at 22 ms; the next poll of the faulted wait (10 ms after the fault, at 30 ms)
    // applies it: the reply is `reloaded`, the resource is Running without a fault, and the candidate
    // scans at once. Wrong behaviour caught: a change that is refused or waits forever while faulted; a
    // fault that survives the change.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.pass_at(&mut runner, 0);
    rig.pass_at(&mut runner, 10);
    rig.faults
        .borrow_mut()
        .push_back(RuntimeError::DivisionByZero);
    rig.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert!(runner.last_error().is_some());

    assert_eq!(rig.call_at(&mut runner, 22, &reload(1, GPIO)), no_replies());
    rig.pass_at(&mut runner, 30);
    assert_eq!(rig.replies(), [reloaded(1, 1)]);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);
    assert!(entries(&rig.log).ends_with(&texts(&["commit", "drop running", "scan change 1 30",])));
}

#[test]
fn a_stop_drops_the_waiting_change_and_it_is_never_applied() {
    // Specification 11 §6.7.1: a reload the scan thread drops unapplied because it stopped first is
    // reported as `resource stopped before the reload was applied` (`resource_not_running`). A
    // `shutdown` and a change arrive together at 15 ms: the stop is answered, the change is queued, and
    // the boundary at 20 ms takes the stop first, so the change is never prepared. Wrong behaviour
    // caught: a change applied to a program that was told to stop; a reply that is never written.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.pass_at(&mut runner, 0);
    rig.pass_at(&mut runner, 10);
    rig.ports.link.send(&request(1, "shutdown"));
    rig.ports.link.send(&reload(2, GPIO));
    rig.pass_at(&mut runner, 15);
    let first = rig.replies();
    assert_eq!(first.len(), 1, "only the stop is answered: {first:?}");
    assert_eq!(first[0]["result"], json!({"status": "stopping"}));

    rig.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    assert_eq!(
        rig.replies(),
        [json!({
            "id": 2,
            "ok": false,
            "error": "resource stopped before the reload was applied",
            "error_code": "resource_not_running",
        })]
    );
    assert!(!entries(&rig.log).contains(&String::from("prepare")));
}

#[test]
fn a_reload_withdrawn_while_the_debugger_holds_the_scan_lets_the_hold_go_on() {
    // Specification 34 §11.3 and STATE-08: a scan that the debugger holds reaches no boundary, so a
    // change that waits for one is withdrawn after five seconds with `reload timeout`, and the loop
    // reads requests again, so that a `resume` can be served. The board probe's scan at 10 ms stops at
    // `led := button;`. A change arrives at 1,200 ms, so it is withdrawn at 6,200 ms; the hold's wait
    // ends exactly then, though the hardware watchdog's feed interval is a minute. A `status` at
    // 6,500 ms and a `resume` at 7,000 ms are served after the reply to the change, in this order, and
    // the change is never prepared. Wrong behaviour caught: a hold that never withdraws the change,
    // which keeps the loop from reading the `resume` and holds the scan for good; a withdrawal late by
    // the feed interval; a change that is applied while the scan is held.
    let rig = Rig::new();
    let platform = TestPlatform {
        feed_interval: core::time::Duration::from_secs(60),
        ..TestPlatform::default()
    };
    let mut runner = rig.runner_holding(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        platform,
    );
    runner.shell().debug.add_breakpoint(on_led_assignment());
    rig.ports.wait.advance_time();
    rig.ports.arrive_at(ms(1_200), &reload(1, GPIO));
    rig.ports.arrive_at(ms(6_500), &request(2, "status"));
    rig.ports.arrive_at(ms(7_000), &request(3, "resume"));

    runner.run_pass().expect("the test wait does not fail");
    runner.run_pass().expect("the test wait does not fail");

    let replies = rig.replies();
    assert_eq!(replies.len(), 3, "one reply to each request: {replies:?}");
    assert_eq!(
        replies[0],
        json!({"id": 1, "ok": false, "error": "reload timeout"}),
        "the change is withdrawn first, as a computer's is"
    );
    assert_eq!(replies[1]["id"], json!(2));
    assert_eq!(replies[1]["result"]["state"], json!("paused"));
    assert_eq!(
        replies[1]["result"]["debug_hold"],
        json!({"reason": "breakpoint"}),
        "the scan was held all that time"
    );
    assert_eq!(replies[2]["result"], json!({"status": "running"}));
    assert!(
        rig.ports.wait.take_deadlines().contains(&ms(6_200)),
        "the hold waited for the moment the change is withdrawn"
    );
    assert!(!entries(&rig.log).contains(&String::from("prepare")));
    assert_eq!(runner.resource_state(), ResourceState::Running);
}

#[test]
fn a_paused_program_takes_an_online_change_at_its_next_interval_and_stays_paused() {
    // Specification 34 §11.3 and STATE-08: while the program is paused in production mode its boundary
    // comes one cycle interval after the last, and the change is applied there; the candidate takes the
    // pause over, so it runs no scan until a `resume`. The probe is paused by a request at 12 ms, which
    // takes effect at the boundary at 20 ms; the change arrives at 25 ms and is applied at the boundary
    // at 30 ms, where the candidate's task timing starts and no scan runs; the `resume` at 35 ms takes
    // effect at the boundary at 40 ms, where the candidate scans. Wrong behaviour caught: a change that
    // waits for a scan a paused program never runs, and times out; a change that resumes the program.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.pass_at(&mut runner, 0);
    rig.pass_at(&mut runner, 10);
    let paused = rig.call_at(&mut runner, 12, &request(1, "pause"));
    assert_eq!(paused[0]["result"], json!({"status": "paused"}));
    rig.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Paused);

    assert_eq!(rig.call_at(&mut runner, 25, &reload(2, GPIO)), no_replies());
    rig.pass_at(&mut runner, 30);
    let replies = rig.replies();
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], json!(2));
    assert_eq!(replies[0]["result"], json!({"status": "reloaded"}));
    assert!(
        entries(&rig.log).ends_with(&texts(&[
            "prepare",
            "checkpoint reads 0",
            "commit",
            "drop running",
        ])),
        "the change is applied and the paused candidate runs no scan: {:?}",
        entries(&rig.log)
    );
    assert_eq!(runner.resource_state(), ResourceState::Paused);

    let resumed = rig.call_at(&mut runner, 35, &request(3, "resume"));
    assert_eq!(resumed[0]["result"], json!({"status": "running"}));
    rig.pass_at(&mut runner, 40);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert!(entries(&rig.log).ends_with(&texts(&["scan change 1 40"])));
}

#[test]
fn a_restart_that_waits_for_the_boundary_acts_on_the_program_the_change_gave() {
    // Specification 34 §11.3 and STATE-08: at a boundary the online change is taken before a pending
    // restart, as a computer's scan thread drains its commands, so the restart restarts the program the
    // change gave. A cold `restart` is queued at 12 ms and a change at 14 ms; the boundary at 20 ms
    // prepares and commits the candidate, drops the running program, restarts the candidate cold at
    // once and starts its task timing again, and then the candidate scans. Wrong behaviour caught: a
    // restart taken first, which restarts the program the change then replaces; a restart that is lost.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.pass_at(&mut runner, 0);
    rig.pass_at(&mut runner, 10);
    let queued = rig.call_at(
        &mut runner,
        12,
        &json!({"id": 1, "type": "restart", "params": {"mode": "cold"}}),
    );
    assert_eq!(queued[0]["result"], json!({"status": "restart queued"}));
    assert_eq!(rig.call_at(&mut runner, 14, &reload(2, GPIO)), no_replies());

    rig.pass_at(&mut runner, 20);
    let replies = rig.replies();
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["result"], json!({"status": "reloaded"}));
    assert!(
        entries(&rig.log).ends_with(&texts(&[
            "prepare",
            "checkpoint reads 0",
            "commit",
            "drop running",
            "restart change 1 Cold",
            "timing change 1 20",
            "scan change 1 20",
        ])),
        "the change first, then the restart of the candidate: {:?}",
        entries(&rig.log)
    );
}

#[test]
fn an_online_change_installs_the_next_generation_and_its_live_values_carry_it() {
    // Specification 34 §11.3, UPDATE-04 and §4.3.1: the change installs the next generation of the
    // running program's lineage. A reference taken before it is refused as stale after it, and a
    // reference taken after it reads. The candidate's Live Values carry the installed generation, so
    // a force that names an instance by a bare identity after the change is resolved in it. The board
    // probe starts in the first generation with `probe_depth` at its declared 1; the change at 15 ms
    // is committed at 20 ms. Wrong behaviour caught: a candidate that starts the first generation
    // again, which would take a stale identity for a fresh one; Live Values that stay at the first
    // generation, which would refuse every instance force after the change.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.pass_at(&mut runner, 0);
    rig.pass_at(&mut runner, 10);
    let Program::Installed(running) = &runner.cycle else {
        panic!("a program is installed");
    };
    let stale = running
        .engine
        .storage()
        .ref_for_global("probe_depth")
        .expect("the probe declares probe_depth");
    assert_eq!(running.engine.generation(), Generation::FIRST);
    assert_eq!(runner.live.generation(), Generation::FIRST);

    assert_eq!(rig.call_at(&mut runner, 15, &reload(1, GPIO)), no_replies());
    rig.pass_at(&mut runner, 20);
    assert_eq!(rig.replies(), [reloaded(1, 1)]);

    let second = Generation::FIRST.next().expect("a second generation");
    assert_eq!(runner.live.generation(), second, "the Live Values");
    let Program::Installed(candidate) = &mut runner.cycle else {
        panic!("a program is installed");
    };
    assert_eq!(candidate.engine.generation(), second, "the candidate");
    assert_eq!(
        candidate.engine.read_reference(&stale),
        Err(RuntimeError::ReferenceLifetime),
        "a reference taken before the change is stale"
    );
    let fresh = candidate
        .engine
        .storage()
        .ref_for_global("probe_depth")
        .expect("the candidate declares probe_depth");
    assert_eq!(fresh.generation, second);
    assert_eq!(
        candidate.engine.read_reference(&fresh),
        Ok(Retained::Int(1))
    );
}
