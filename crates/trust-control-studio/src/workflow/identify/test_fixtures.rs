//! The fixtures of the connected run's tests (`control-studio-identification-recovery.test.ts`): the tank loop and its live sample, the
//! recording owner and the journals of a dispatched step and of a baseline, the journal folders, and the check of the refusal that a held
//! plant input gives. The fake runtime of `test_support` takes the owner from here and re-exports the rest.

use std::fs;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde_json::json;
use trust_control::experiment::{
    ControlEvidenceSource, ControlExperimentPlan, ControlExperimentRecovery, ControllerMode,
    ExperimentSample, PlanSignal, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::baseline::BaselineSummary;
use trust_control::identification::campaign::journal::IdentificationCampaignJournal;
use trust_control::identification::texts::MISSING_NONZERO_STEP;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::signals::Quality;

use crate::journal::{Journal, JournalEntry};

/// The tank of the `TypeScript` tests (`tankLoop`) at `revision`: a setpoint of 45 degC measured in 0 to 100 degC, driven by a
/// 0 to 100 % output, with a 100 ms task and a 600 s maximum experiment, identified by Automatic.
pub(crate) fn tank_loop(revision: &str) -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": revision,
        "projectId": "project", "runtimeId": "runtime",
        "controller": { "instance": "Main.TIC101", "type": "Control.PID", "revision": "sha256:test",
            "action": "direct", "taskPeriodMs": 100 },
        "signals": {
            "setpoint": { "symbol": "Main.TIC101_Setpoint", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "Main.TT101.Value", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "Main.TIC101_Output", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "experiment": { "execution": "control-studio", "kind": "observation", "amplitude": 0,
            "durationSeconds": 600, "seed": 1 }
    }))
    .expect("the tank loop")
}

/// The live sample of the tank in Auto (`liveSample("Auto")`): the setpoint and the measurement at 45 degC, the output at
/// 30 %, on a simulated target.
pub(crate) fn live_sample() -> ExperimentSample {
    let signal = |symbol: &str, value: f64, unit: &str| PlanSignal {
        symbol: symbol.to_owned(),
        address: None,
        value,
        unit: unit.to_owned(),
        quality: Quality::Good,
    };
    ExperimentSample {
        scan: 7,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("Main.TIC101_Setpoint", 45.0, "degC")),
        process_variable: signal("Main.TT101.Value", 45.0, "degC"),
        manipulated_variable: signal("Main.TIC101_Output", 30.0, "%"),
        additional: Vec::new(),
    }
}

/// The owner of the tank's recording `recording_id`: its setpoint, measurement and manipulated variable by symbol, the binding
/// list a recording of the tank holds (E11.2).
pub(super) fn owner(recording_id: &str) -> RecordingRecoveryOwner {
    let binding = |id: &str, symbol: &str| RecordingRecoveryBinding {
        id: id.to_owned(),
        address: None,
        symbol: Some(symbol.to_owned()),
        iec_type: "LREAL".to_owned(),
    };
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: recording_id.to_owned(),
        project_id: "project".to_owned(),
        runtime_id: "runtime".to_owned(),
        loop_id: "tank".to_owned(),
        loop_revision: "rev-1".to_owned(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some("program".to_owned()),
        start_acknowledgement_unconfirmed: None,
        bindings: vec![
            binding("setpoint", "Main.TIC101_Setpoint"),
            binding("processVariable", "Main.TT101.Value"),
            binding("manipulatedVariable", "Main.TIC101_Output"),
        ],
    }
}

/// A journaled setpoint step of the tank (`planFor`): from 45 to 47 degC, held for `duration_ms`.
pub(crate) fn plan(experiment_id: &str, duration_ms: u64) -> ControlExperimentPlan {
    serde_json::from_value(json!({
        "id": experiment_id, "source": "simulation", "kind": "setpoint_step",
        "affectedSignal": "Main.TIC101_Setpoint", "valueType": "LREAL", "from": 45, "to": 47, "unit": "degC",
        "minimum": 0, "maximum": 100, "durationMs": duration_ms, "expectedScan": 300, "task": "$background",
        "stopConditions": ["Abort"], "restorationValue": 45, "requiresPhysicalApproval": false,
        "trajectory": [{ "atMs": 0, "value": 47 }]
    }))
    .expect("the plan")
}

/// The steady baseline of a journaled step (`journalFor`): 45 degC, its noise and its half range, one sample a second and a
/// 30 s window.
fn baseline() -> BaselineSummary {
    BaselineSummary {
        mean: 45.0,
        noise: 0.03,
        half_range: 0.05,
        spacing_seconds: 1.0,
        window_seconds: 30.0,
        input_mean: None,
        input_noise: None,
    }
}

/// A journal folder of its own for one test.
pub(crate) fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "trust-identify-recovery-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&folder);
    folder
}

/// The tank's journal in `folder`, empty.
pub(crate) fn empty_journal(folder: &Path) -> Mutex<Journal> {
    Mutex::new(Journal::open(folder, "project", "tank").expect("an empty journal"))
}

/// The tank's journal as a reopened panel reads it.
pub(crate) fn reopened(folder: &Path) -> Vec<JournalEntry> {
    Journal::open(folder, "project", "tank")
        .expect("the journal reopens")
        .entries()
        .to_vec()
}

/// The tank's journal holding a dispatched step (`journalFor`): the campaign of recording-1 at loop revision rev-1 with the
/// plan of `step`, its baseline, the process time its step started at and the plant inputs that follow it, and the
/// experiment's own record (E11.2).
pub(crate) fn journaled_step(
    folder: &Path,
    step: &ControlExperimentPlan,
    step_start_seconds: f64,
    channels_remaining: u64,
) -> Mutex<Journal> {
    let journal = empty_journal(folder);
    journal
        .lock()
        .persist(JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::step_phase(
                "rev-1",
                owner("recording-1"),
                step.clone(),
                baseline(),
                step_start_seconds,
                channels_remaining,
            )),
        })
        .expect("the step is journaled");
    journal
        .lock()
        .persist(JournalEntry::Experiment {
            recovery: Box::new(ControlExperimentRecovery {
                schema_version: 2,
                experiment_id: step.id.clone(),
                project_id: "project".to_owned(),
                loop_id: "tank".to_owned(),
                loop_revision: "rev-1".to_owned(),
                plan: Some(step.clone()),
                recording_owner: Some(owner("recording-1")),
            }),
        })
        .expect("the experiment is journaled");
    journal
}

/// The tank's journal holding a campaign still in its baseline: its recording, and no step dispatched.
pub(crate) fn journaled_baseline(folder: &Path) -> Mutex<Journal> {
    let journal = empty_journal(folder);
    journal
        .lock()
        .persist(JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::baseline_phase(
                "rev-1",
                owner("recording-1"),
                1,
            )),
        })
        .expect("the baseline is journaled");
    journal
}

/// A failure's whole text: its detail, or its message when it has no more to say.
pub(crate) fn whole_text(failure: &Failure) -> &str {
    failure.detail.as_deref().unwrap_or(&failure.message)
}

/// Checks the answer of the fit to the tank's recording, whose plant input stays at 30 % while the setpoint steps. A
/// closed-loop record is fitted from the recorded plant input (B3.5), which does not move, so the excitation fit
/// refuses it as the missing nonzero step (N2.2, N2.6), and another method is the fix (B3.9).
pub(crate) fn assert_no_model_of_a_held_input(failure: &Failure) {
    assert_eq!(
        failure.message,
        "The recording is missing a nonzero step: the plant input never changes, so there is no input change to fit.",
        "{failure:?}"
    );
    assert_eq!(failure.detail.as_deref(), Some(MISSING_NONZERO_STEP));
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Unclassified, NextAction::ChooseMethod),
        "{failure:?}"
    );
}
