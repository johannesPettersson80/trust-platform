//! Tests of Identify's mode change (`connectedIdentificationMode.ts`; specification 35, A6.2, A8.5, A8.6 and B3.4 step 1; runtime
//! protocol annex E6, E11.2, E11.4 and E11.9) on the transfer fake of `test_support`, alone, inside a connected run on the
//! experiment fake of `identify::test_support`, and through the recovery after a restart of the engine. The `TypeScript` tests
//! replaced the mode by a double that recorded `enter-manual` and `restore`; here the transfers the runtime received stand for
//! those calls, in their order with the recording and the fit.

#![allow(clippy::float_cmp)] // The allowance of a transfer to Manual is the exact check of 1e-6.

use std::collections::BTreeMap;
use std::fs;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::commissioning::checkpoint::{
    CheckpointStage, CommissioningCheckpoint, IdentificationModeCheckpoint, ModeTransferOwner,
};
use trust_control::commissioning::commissioning_result::TransferOwner;
use trust_control::commissioning::parameter_application::ParameterOwner;
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::commissioning::transfer::{
    pid_transfer_binding, TransferBinding, TransferMember, TransferValue,
};
use trust_control::commissioning::transfer_tracking::{parallel_pid_tracking, TransferTracking};
use trust_control::experiment::ControlExperimentKind;
use trust_control::failure::NextAction;
use trust_control::identification::campaign::SILENT_RUNTIME_TEXT;
use trust_control::identification::connected_run::{identification_cancelled, FITTING_TEXT};
use trust_control::identification::mode_transfer::mode_binding;
use trust_control::identification::texts::MODE_TRANSFER_UNAVAILABLE;
use trust_control::model::general::decode_general_model;
use trust_control::validation::engineer_verification_plan::engineer_requirements;

use super::test_support::{
    pid_tank_loop, run_with_transfers, LoggedReport, LoggedStudy, ModeRuntime, TransferFake,
    MODE_SIGNAL,
};
use super::IdentificationMode;
use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::RuntimeClient;
use crate::workflow::identify::test_support::{
    folder, journaled_step, passes, plan, reopened, run_tank, whole_text, FakeClock, FakeState,
    StartBehaviour, Study,
};
use crate::workflow::identify::{IdentifyRequest, RunPurpose};
use crate::workflow::identify_recovery::{cancel_operation_on, resume, RecoveryPorts};

/// The transfer that enters Manual on the mode binding.
const ENTER_MANUAL: &str = "transfer.apply tank:identification-mode manual";
/// The transfer that enters Auto on the mode binding.
const ENTER_AUTO: &str = "transfer.apply tank:identification-mode auto";
/// The transfer that writes the references and commands back in Manual.
const COMMANDS: &str = "transfer.apply tank:identification-mode:commands manual";

/// The tank's journal in `folder`, as the engine opens it.
fn journal_in(folder: &std::path::Path) -> Mutex<Journal> {
    Mutex::new(Journal::open(folder, "project", "tank").expect("the tank's journal"))
}

/// The value of a request's field as a map of numbers, decoded as the engine decodes numbers.
fn numbers(value: &Value) -> BTreeMap<String, f64> {
    trust_control::json::decode_value(value.clone()).expect("a map of numbers")
}

/// The binding an apply request names, decoded as the engine decodes numbers.
fn binding_of(apply: &Value) -> TransferBinding {
    trust_control::json::decode_value(apply["binding"].clone()).expect("the apply's binding")
}

/// The symbols of `members`, in order.
fn symbols(members: &[TransferMember]) -> Vec<&str> {
    members
        .iter()
        .map(|member| member.symbol.as_str())
        .collect()
}

/// The identifier of the mode checkpoint `journal` holds.
fn mode_operation(journal: &Mutex<Journal>) -> String {
    journal
        .lock()
        .entries()
        .iter()
        .find_map(|entry| {
            if let JournalEntry::IdentificationMode { operation_id, .. } = entry {
                Some(operation_id.clone())
            } else {
                None
            }
        })
        .expect("the mode checkpoint is journaled")
}

/// B3.4 step 1, A6.3 and E11.2: a plant-input method on a loop in Auto enters Manual through one transfer of the mode binding,
/// which writes every state as the runtime holds it and carries the declared states unaligned, so the Manual command takes the
/// held output of 30 %. The mode found is journaled as the checkpoint before anything is sent, the transfer's owner is in the
/// journal before its apply reaches the runtime, and the confirmed owner is let go. A transfer to Manual allows 1e-6 (E6.9).
#[test]
fn entering_manual_journals_the_mode_found_and_the_transfers_owner_before_its_apply() {
    let folder = folder("mode-enter");
    let journal = journal_in(&folder);
    let transfers = TransferFake::start(true);
    transfers.watch_journal(&folder);
    let loop_definition = pid_tank_loop("rev-1", false);
    let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
        .expect("the tank transfers");
    mode.enter(true).expect("the loop enters Manual");

    assert!(!transfers.auto(), "the loop is in Manual");
    assert_eq!(
        transfers.log(),
        ["journaled tank:identification-mode", ENTER_MANUAL],
        "the owner is journaled before the apply is dispatched"
    );
    let applies = transfers.applies();
    assert_eq!(applies.len(), 1);
    let apply = &applies[0];
    assert_eq!(apply["binding"]["loop_id"], "tank:identification-mode");
    assert_eq!(apply["manual"].as_bool(), Some(true));
    assert_eq!(apply["maximum_output_change"].as_f64(), Some(1e-6));
    assert_eq!(
        numbers(&apply["values"]),
        BTreeMap::from([
            ("Main.TIC101_Setpoint".to_owned(), 45.0),
            ("Main.TIC101_ManualOutput".to_owned(), 12.0),
            ("Main.TIC101_Integral".to_owned(), 30.0),
            ("Main.TIC101_PreviousError".to_owned(), 0.0),
        ])
    );
    assert_eq!(apply["tracking"]["kind"], "declared-state-v1");
    assert_eq!(apply["tracking"]["align_output"].as_bool(), Some(false));
    assert_eq!(
        apply["tracking"]["bounded_state_symbols"],
        json!(["Main.TIC101_Integral", "Main.TIC101_PreviousError"])
    );
    assert_eq!(
        transfers.value("Main.TIC101_ManualOutput"),
        30.0,
        "the Manual command holds the output"
    );
    let entries = reopened(&folder);
    let [JournalEntry::IdentificationMode { checkpoint, .. }] = entries.as_slice() else {
        panic!("the journal holds the mode checkpoint: {entries:?}");
    };
    assert_eq!(checkpoint.loop_revision, "rev-1");
    assert_eq!(
        checkpoint.original.values.get(MODE_SIGNAL),
        Some(&TransferValue::Boolean(true)),
        "the mode found is Auto"
    );
    assert_eq!(checkpoint.transfer, None, "the confirmed owner is let go");
    let _ = fs::remove_dir_all(&folder);
}

/// A6.2, A6.4 and E11.9: the return of a loop found in Auto writes its setpoint and Manual command back in Manual, on the transfer
/// loop of the commands with the record's gains as guards and no tracking, then enters Auto with the states as they are. With the
/// baseline's noise n = 0.05 and no error, the first output may move by the PID's response 2·|Kp|·n + |Ki|·Δt·n =
/// 2·2·0.05 + 0.1·0.1·0.05 = 0.2005. The checkpoint is cleared once the runtime confirmed every transfer.
#[test]
fn the_return_writes_the_commands_back_in_manual_and_enters_auto_with_the_pids_allowance() {
    let folder = folder("mode-return");
    let journal = journal_in(&folder);
    let transfers = TransferFake::start(true);
    let loop_definition = pid_tank_loop("rev-1", false);
    let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
        .expect("the tank transfers");
    mode.enter(true).expect("the loop enters Manual");
    mode.set_process_noise(0.05);
    mode.restore().expect("the loop returns to Auto");

    assert!(transfers.auto(), "the loop is back in Auto");
    assert_eq!(transfers.events(), [ENTER_MANUAL, COMMANDS, ENTER_AUTO]);
    let applies = transfers.applies();
    let commands = binding_of(&applies[1]);
    assert_eq!(
        symbols(&commands.states),
        ["Main.TIC101_Setpoint", "Main.TIC101_ManualOutput"]
    );
    assert_eq!(
        symbols(&commands.guards),
        [
            "Main.TIC101_Parameters.Kp",
            "Main.TIC101_Parameters.Ki",
            "Main.TIC101_Parameters.Kd"
        ]
    );
    assert_eq!(
        numbers(&applies[1]["values"]),
        BTreeMap::from([
            ("Main.TIC101_Setpoint".to_owned(), 45.0),
            ("Main.TIC101_ManualOutput".to_owned(), 12.0),
        ]),
        "the values the loop was found with"
    );
    assert!(
        applies[1].get("tracking").is_none(),
        "the commands carry no tracking"
    );
    // A6.2, E6.9: writing the found Manual command (12) back moves the held output (30) by 18, which is its allowance.
    assert_eq!(applies[1]["maximum_output_change"].as_f64(), Some(18.0));
    assert_eq!(applies[2]["manual"].as_bool(), Some(false));
    let allowance = applies[2]["maximum_output_change"]
        .as_f64()
        .expect("a number");
    assert!((allowance - 0.2005).abs() < 1e-12, "{allowance}");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// B3.4 step 1: a loop already in the mode the method needs is left as it is, in Manual and in Auto: no transfer, no journal
/// entry, and nothing to return.
#[test]
fn a_loop_already_in_the_wanted_mode_has_no_transfer_and_no_journal_entry() {
    let folder = folder("mode-unchanged");
    let journal = journal_in(&folder);
    let loop_definition = pid_tank_loop("rev-1", false);
    for (auto, manual) in [(false, true), (true, false)] {
        let transfers = TransferFake::start(auto);
        let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
            .expect("the tank transfers");
        mode.enter(manual).expect("nothing to change");
        mode.restore().expect("nothing to return");
        assert_eq!(transfers.applies(), Vec::<Value>::new(), "no transfer");
        assert_eq!(transfers.auto(), auto, "the mode is unchanged");
        assert_eq!(
            reopened(&folder),
            Vec::<JournalEntry>::new(),
            "no journal entry"
        );
    }
    let _ = fs::remove_dir_all(&folder);
}

/// E11.4 and A8.6: after a restart of the engine, a mode checkpoint whose transfer was retained but never confirmed is reconciled
/// by the transfer's identity: the cancel names it, the runtime that never received it answers cancelled before dispatch, and no
/// apply carries it again. The loop, still in Auto, then returns as any loop found in Auto does.
#[test]
fn an_owned_transfer_of_an_interrupted_mode_change_is_cancelled_and_never_applied_again() {
    let folder = folder("mode-owned");
    let transfers = TransferFake::start(true);
    let loop_definition = pid_tank_loop("rev-1", false);
    let binding = mode_binding(&loop_definition).expect("the mode binding");
    let original = RuntimeClient::connect(&transfers.connection())
        .expect("connected")
        .transfer_status(&binding)
        .expect("the status");
    Journal::open(&folder, "project", "tank")
        .expect("the tank's journal")
        .persist(JournalEntry::IdentificationMode {
            operation_id: "identification-mode-1".to_owned(),
            checkpoint: Box::new(IdentificationModeCheckpoint {
                loop_revision: "rev-1".to_owned(),
                original,
                transfer: Some(ModeTransferOwner {
                    binding,
                    owner: TransferOwner {
                        id: "transaction-lost".to_owned(),
                        program: "program".to_owned(),
                        binding: "binding:tank:identification-mode".to_owned(),
                    },
                }),
            }),
        })
        .expect("the checkpoint is journaled");
    let journal = journal_in(&folder);
    let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
        .expect("the tank transfers");
    mode.restore().expect("the loop returns to Auto");

    assert_eq!(
        transfers.events(),
        [
            "transfer.cancel tank:identification-mode transaction-lost",
            ENTER_MANUAL,
            COMMANDS,
            ENTER_AUTO
        ]
    );
    assert!(
        transfers
            .applies()
            .iter()
            .all(|apply| apply["transaction_id"] != "transaction-lost"),
        "the interrupted apply is never sent again"
    );
    assert!(transfers.auto(), "the loop is back in Auto");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// A6.7 and E11.5: a program reloaded since the mode was changed refuses the return and names both revisions. Nothing is written
/// to the new program, and the checkpoint stays journaled until a fresh review.
#[test]
fn a_reloaded_program_refuses_the_return_and_keeps_the_checkpoint() {
    let folder = folder("mode-reloaded");
    let journal = journal_in(&folder);
    let transfers = TransferFake::start(true);
    let loop_definition = pid_tank_loop("rev-1", false);
    let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
        .expect("the tank transfers");
    mode.enter(true).expect("the loop enters Manual");
    transfers.reload("program-2");
    let failure = mode.restore().expect_err("the program changed");

    assert_eq!(
        failure.message,
        "The program changed from revision program to program-2; the saved identification mode cannot be restored to this program."
    );
    assert_eq!(failure.next_action, NextAction::None);
    assert_eq!(
        transfers.events(),
        [ENTER_MANUAL],
        "nothing is written to the new program"
    );
    assert_eq!(reopened(&folder).len(), 1, "the checkpoint stays journaled");
    let _ = fs::remove_dir_all(&folder);
}

/// B3.4 step 1, A6.2 and §17.1 (`control-studio-generated-loop.test.ts`, "returns a loop to Auto after identification without
/// writing back the manual output the code sets", inside a run): a plant-input step on a loop in Auto, whose program copies the
/// output into the Manual command, enters Manual before the recording starts and runs its step in Manual. Once the recording is
/// kept the loop returns: its setpoint alone is written back in Manual, then the loop enters Auto. Nothing is left journaled.
#[test]
fn identify_enters_manual_for_a_plant_input_step_and_returns_to_auto_when_the_campaign_ends() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let transfers = TransferFake::start(true);
    let folder = folder("mode-run");
    let journal = journal_in(&folder);
    let loop_definition = pid_tank_loop("rev-1", true);
    // The campaign's own outcome, the fit of its record, is the long-campaign tests'; the mode returns whatever it gave.
    let _ = run_with_transfers(&state, &transfers, &journal, &loop_definition, false);

    assert_eq!(
        transfers.events(),
        [
            ENTER_MANUAL,
            "recording.start",
            "retain",
            COMMANDS,
            ENTER_AUTO
        ]
    );
    let started = state.started_plans();
    assert_eq!(started.len(), 1, "one step");
    assert_eq!(
        started[0].kind,
        ControlExperimentKind::MvStep,
        "the step runs in Manual"
    );
    let applies = transfers.applies();
    let commands = binding_of(&applies[1]);
    assert_eq!(
        symbols(&commands.states),
        ["Main.TIC101_Setpoint"],
        "the Manual command the program sets is not written back"
    );
    assert!(transfers.auto(), "the loop is back in Auto");
    assert_eq!(
        reopened(&folder),
        Vec::<JournalEntry>::new(),
        "nothing is left journaled"
    );
    let _ = fs::remove_dir_all(&folder);
}

/// B3.4 step 1: a runtime connection that takes no controller transfers cannot move the loop to the mode a method needs, so a
/// plant-input step on a loop in Auto is refused before anything is recorded, written or journaled.
#[test]
fn a_runtime_without_transfers_refuses_a_method_that_needs_the_other_mode() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let folder = folder("mode-no-transfers");
    let journal = journal_in(&folder);
    let loop_definition = pid_tank_loop("rev-1", false);
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let failure = run_tank(
        &state,
        &journal,
        &mut Study::new(&state),
        &request,
        RunPurpose::Identify,
    )
    .expect_err("the runtime takes no transfers");

    assert_eq!(failure.message, MODE_TRANSFER_UNAVAILABLE);
    assert_eq!(
        state.calls(),
        Vec::<String>::new(),
        "nothing is recorded, started or reported"
    );
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// A6.2 (the Cancel of `ConnectedIdentification`): a campaign cancelled once its mode is entered records nothing, and the loop
/// still returns to the mode it was found in.
#[test]
fn a_cancelled_campaign_returns_the_loop_to_the_mode_it_was_found_in() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let transfers = TransferFake::start(true);
    let folder = folder("mode-cancelled");
    let journal = journal_in(&folder);
    let loop_definition = pid_tank_loop("rev-1", false);
    let outcome = run_with_transfers(&state, &transfers, &journal, &loop_definition, true);

    assert_eq!(outcome.err(), Some(identification_cancelled()));
    assert_eq!(transfers.events(), [ENTER_MANUAL, COMMANDS, ENTER_AUTO]);
    assert_eq!(
        state.calls(),
        Vec::<String>::new(),
        "nothing is recorded or started"
    );
    assert!(transfers.auto(), "the loop is back in Auto");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// A6.2 and A3.4 (`control-studio-long-process-campaign.test.ts`, "the five-second supervision rule stops a campaign that
/// receives no samples, and restores it"): a campaign whose runtime sends no sample fails; its recording is kept, and then the
/// loop returns to the mode it was found in.
#[test]
fn a_failed_campaign_keeps_its_recording_and_returns_the_mode() {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    state.silence_follows();
    let transfers = TransferFake::start(true);
    let folder = folder("mode-failed");
    let journal = journal_in(&folder);
    let loop_definition = pid_tank_loop("rev-1", false);
    let Err(failure) = run_with_transfers(&state, &transfers, &journal, &loop_definition, false)
    else {
        panic!("the runtime sends no samples");
    };

    assert_eq!(whole_text(&failure), SILENT_RUNTIME_TEXT);
    assert_eq!(
        transfers.events(),
        [
            ENTER_MANUAL,
            "recording.start",
            "retain",
            COMMANDS,
            ENTER_AUTO
        ]
    );
    assert!(transfers.auto(), "the loop is back in Auto");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// E11.4 and A6.2 (`control-studio-identification-recovery.test.ts`, "a start the runtime holds as running is followed to its
/// end", "a start the runtime holds as already ended is concluded" and "a start the runtime never received is cleared"): whatever
/// the identity of a start whose acknowledgement was lost, the start is sent once and nothing is aborted, and once no experiment
/// of the run is left running the loop returns to the mode it was found in, with nothing left journaled.
#[test]
fn a_start_reconciled_by_its_identity_returns_the_mode_once_nothing_is_left_running() {
    for behaviour in [
        StartBehaviour::Running,
        StartBehaviour::Terminal,
        StartBehaviour::Absent,
    ] {
        let state = FakeState::new(behaviour, 0);
        let transfers = TransferFake::start(true);
        let folder = folder(&format!("mode-start-{behaviour:?}"));
        let journal = journal_in(&folder);
        let loop_definition = pid_tank_loop("rev-1", false);
        let _ = run_with_transfers(&state, &transfers, &journal, &loop_definition, false);

        assert_eq!(
            transfers.events(),
            [
                ENTER_MANUAL,
                "recording.start",
                "retain",
                COMMANDS,
                ENTER_AUTO
            ],
            "{behaviour:?}"
        );
        assert_eq!(
            state.count("experiment.start"),
            1,
            "one start ({behaviour:?})"
        );
        assert_eq!(
            state.count("experiment.abort"),
            0,
            "no abort ({behaviour:?})"
        );
        assert!(transfers.auto(), "the loop is back in Auto ({behaviour:?})");
        assert_eq!(
            reopened(&folder),
            Vec::<JournalEntry>::new(),
            "{behaviour:?}"
        );
        let _ = fs::remove_dir_all(&folder);
    }
}

/// E11.4 and A8.6 (`control-studio-identification-recovery.test.ts`, "a start whose status cannot be read keeps its journal and
/// mode for reopening, and sends nothing further"): the mode is not returned underneath an experiment whose state is unknown,
/// and its checkpoint stays journaled with the campaign for the reconciliation on reopening.
#[test]
fn an_experiment_whose_state_is_unknown_keeps_the_mode_and_its_checkpoint() {
    let state = FakeState::new(StartBehaviour::Unreadable, 0);
    let transfers = TransferFake::start(true);
    let folder = folder("mode-unresolved");
    let journal = journal_in(&folder);
    let loop_definition = pid_tank_loop("rev-1", false);
    let outcome = run_with_transfers(&state, &transfers, &journal, &loop_definition, false);

    assert!(outcome.is_err(), "the start cannot be reconciled");
    assert_eq!(transfers.events(), [ENTER_MANUAL, "recording.start"]);
    assert!(!transfers.auto(), "the loop stays in Manual");
    assert!(
        reopened(&folder)
            .iter()
            .any(|entry| matches!(entry, JournalEntry::IdentificationMode { .. })),
        "the mode checkpoint stays journaled"
    );
    let _ = fs::remove_dir_all(&folder);
}

/// A8.6 and §0.1b `cancel`: an engine that stopped after entering Manual leaves the mode checkpoint journaled. After the restart,
/// the Cancel of that operation returns the loop to Auto, the commands in Manual first, and clears the checkpoint.
#[test]
fn after_a_restart_the_cancel_of_the_journaled_mode_returns_the_loop_to_auto() {
    let folder = folder("mode-restart");
    let transfers = TransferFake::start(true);
    let loop_definition = pid_tank_loop("rev-1", false);
    {
        let journal = journal_in(&folder);
        let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
            .expect("the tank transfers");
        mode.enter(true).expect("the loop enters Manual");
    }
    let journal = journal_in(&folder);
    let operation_id = mode_operation(&journal);
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let runtime = ModeRuntime::new(&state, &transfers);
    let mut study = LoggedStudy::new(&state, &transfers);
    cancel_operation_on(
        &runtime,
        &FakeClock::new(&state),
        &journal,
        &loop_definition,
        &operation_id,
        &mut study,
    )
    .expect("the Cancel returns the mode");

    assert_eq!(transfers.events(), [ENTER_MANUAL, COMMANDS, ENTER_AUTO]);
    assert!(transfers.auto(), "the loop is back in Auto");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// A6.2 and A8.6 (`control-studio-identification-recovery.test.ts`, "reopening reattaches a running experiment, finishes it once
/// settled, returns the mode, then fits, and never aborts it"): a setpoint step on a loop found in Manual entered Auto before the
/// restart. Reopening follows the experiment to its settled end and keeps the recording, returns the loop to Manual with its
/// commands, and only then fits.
#[test]
fn reopening_follows_the_experiment_returns_the_mode_and_then_fits() {
    let folder = folder("mode-resume");
    let transfers = TransferFake::start(false);
    let loop_definition = pid_tank_loop("rev-1", false);
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    {
        let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
            .expect("the tank transfers");
        mode.enter(false)
            .expect("the loop enters Auto for its setpoint step");
    }
    let state = FakeState::new(StartBehaviour::Acknowledged, 30_000);
    state.restart("experiment-1", 30_000, 540_000, 2.0);
    let runtime = ModeRuntime::new(&state, &transfers);
    let report = LoggedReport::new(&transfers, false);
    let mut study = LoggedStudy::new(&state, &transfers);
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let resumed = resume(
        RecoveryPorts {
            runtime: &runtime,
            retention: &mut study,
            clock: &FakeClock::new(&state),
            report: &report,
            journal: &journal,
        },
        &request,
    );

    assert!(
        resumed.is_err(),
        "the fit of the fake's constant plant input finds no model"
    );
    assert_eq!(
        transfers.events(),
        [ENTER_AUTO, "retain", ENTER_MANUAL, COMMANDS],
        "the loop found in Manual returns to Manual once the recording is kept"
    );
    let log = transfers.log();
    let position = |entry: &str| log.iter().position(|logged| logged == entry);
    let returned = position(COMMANDS).expect("the commands are written back");
    let fitted = position(&format!("progress {FITTING_TEXT}")).expect("the record is fitted");
    assert!(
        returned < fitted,
        "the mode returns before the fit: {log:?}"
    );
    assert_eq!(state.count("experiment.abort"), 0, "nothing is aborted");
    assert!(!transfers.auto(), "the loop is back in Manual");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// A6.2 and A8.6 (`control-studio-identification-recovery.test.ts`, "reopening an identification whose experiment never started
/// clears its journal, restores the mode and completes the recovery"): the experiment of the journaled step never reached the
/// runtime, so reopening keeps the recording, clears the journal and returns the loop to Manual, the mode it was found in, without
/// a start or an abort.
#[test]
fn reopening_an_identification_whose_experiment_never_started_returns_the_mode() {
    let folder = folder("mode-resume-absent");
    let transfers = TransferFake::start(false);
    let loop_definition = pid_tank_loop("rev-1", false);
    let journal = journaled_step(&folder, &plan("experiment-1", 540_000), 30.0, 0);
    {
        let mut mode = IdentificationMode::new(&loop_definition, transfers.connection(), &journal)
            .expect("the tank transfers");
        mode.enter(false)
            .expect("the loop enters Auto for its setpoint step");
    }
    let state = FakeState::new(StartBehaviour::Absent, 30_000);
    let runtime = ModeRuntime::new(&state, &transfers);
    let report = LoggedReport::new(&transfers, false);
    let mut study = LoggedStudy::new(&state, &transfers);
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let Err(failure) = resume(
        RecoveryPorts {
            runtime: &runtime,
            retention: &mut study,
            clock: &FakeClock::new(&state),
            report: &report,
            journal: &journal,
        },
        &request,
    ) else {
        panic!("the experiment never ran");
    };

    assert!(
        whole_text(&failure).starts_with("The interrupted experiment never started"),
        "{failure:?}"
    );
    assert_eq!(
        transfers.events(),
        [ENTER_AUTO, "retain", ENTER_MANUAL, COMMANDS]
    );
    assert_eq!(
        (
            state.count("experiment.start"),
            state.count("experiment.abort")
        ),
        (0, 0),
        "no start and no abort on reopening"
    );
    assert!(!transfers.auto(), "the loop is back in Manual");
    assert_eq!(reopened(&folder), Vec::<JournalEntry>::new());
    let _ = fs::remove_dir_all(&folder);
}

/// §0.1b `cancel` and B6.4: a journaled Apply is not an identification operation, so Cancel refuses it and names Restore previous
/// settings, which ends it; the checkpoint stays journaled.
#[test]
fn the_cancel_of_a_journaled_apply_is_refused_and_names_restore() {
    let folder = folder("mode-apply-cancel");
    let journal = journal_in(&folder);
    let loop_definition = pid_tank_loop("rev-1", false);
    let binding = pid_transfer_binding(&loop_definition).expect("the PID binding");
    let tracking =
        parallel_pid_tracking(&loop_definition, &binding, false).expect("the PID tracking");
    let checkpoint = CommissioningCheckpoint {
        id: "checkpoint-1".to_owned(),
        loop_revision: "rev-1".to_owned(),
        stage: CheckpointStage::Completed,
        model: decode_general_model(&json!({ "kind": "transfer-function", "revision": "model:1",
            "source": "provided", "validation": "validated", "evidence": ["bench"], "delaySeconds": 0,
            "numerator": [1], "denominator": [1, 1],
            "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
            "outputs": [{ "id": "y", "unit": "degC", "operatingRange": { "minimum": 0, "maximum": 100 } }],
            "operatingPoint": { "inputs": [30], "outputs": [45] } }))
        .expect("the model"),
        task: engineer_requirements(&json!({ "referenceChange": 1, "maximumError": 0.1,
            "settlingBand": 0.02, "settlingTimeSeconds": 2, "durationSeconds": 4 }))
        .expect("the requirements"),
        origin: ParameterStatus {
            supports_request_id: Some(true),
            transaction_id: None,
            program_revision: "program".to_owned(),
            binding_revision: "binding".to_owned(),
            active_revision: "active".to_owned(),
            values: BTreeMap::new(),
            status: RecordState::Applied,
            previous_revision: None,
            previous_values: None,
            reason: None,
        },
        candidate: BTreeMap::new(),
        parameter_operation: ParameterOwner {
            id: "parameter-1".to_owned(),
            program: "program".to_owned(),
            binding: "binding".to_owned(),
        },
        references: BTreeMap::new(),
        binding,
        recovery_tracking: TransferTracking::ParallelPid(tracking),
        tolerance: 0.1,
        replacement_program_revision: None,
        transfer: None,
        recording: None,
        recording_result: None,
        found_mode: None,
        process_noise: None,
        validation_identity: "validation-1".to_owned(),
        transfers: Vec::new(),
    };
    journal
        .lock()
        .persist(JournalEntry::Commissioning {
            checkpoint: Box::new(checkpoint),
        })
        .expect("the Apply is journaled");
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let transfers = TransferFake::start(true);
    let runtime = ModeRuntime::new(&state, &transfers);
    let mut study = LoggedStudy::new(&state, &transfers);
    let failure = cancel_operation_on(
        &runtime,
        &FakeClock::new(&state),
        &journal,
        &loop_definition,
        "checkpoint-1",
        &mut study,
    )
    .expect_err("an Apply is ended by Restore");

    assert_eq!(
        failure.message,
        "Operation checkpoint-1 is an Apply of controller settings, which Cancel does not end."
    );
    assert_eq!(failure.next_action, NextAction::Restore);
    assert_eq!(
        transfers.applies(),
        Vec::<Value>::new(),
        "nothing is written"
    );
    assert_eq!(
        journal
            .lock()
            .entries()
            .iter()
            .map(JournalEntry::operation_id)
            .collect::<Vec<_>>(),
        ["checkpoint-1"],
        "the Apply stays journaled"
    );
    let _ = fs::remove_dir_all(&folder);
}
