//! Control Studio's transactions against a real runtime (checklist V4.3; runtime protocol annex E0.2, E6, E7; specification 35, A6.2 and
//! A6.3). The lesson 30 loop (`examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`, a `Control.PID` with declared states) runs in
//! `trust-runtime run --simulation` at its task period of 100 ms, that is at ×1 real time. The journey transfers the loop to Manual with its
//! recovery tracking, applies a parameter record in Manual and rolls it back, transfers the loop back to Auto with the tracking of its
//! profile, and cancels an identifier that the runtime does not hold. Each step is confirmed by the runtime's own status before the next one.
#![cfg(unix)]
#![allow(clippy::float_cmp)] // The journey compares the values the runtime reports with the values it was given: exact equality is the rule.
                             // The helpers are shared with the other real-runtime tests, which use the items this journey does not.
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::json;
use trust_control::commissioning::adapters::controller_adapter;
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::commissioning_result::{
    TransferOwner, TransferState, TransferStatus,
};
use trust_control::commissioning::parameter_status::RecordState;
use trust_control::commissioning::transfer::{TransferBinding, TransferValue};
use trust_control::commissioning::transfer_protocol::TransferCancelRequest;
use trust_control::failure::Failure;
use trust_control::loop_definition::{LoopDefinition, ParameterBinding};
use trust_control::model::general::{decode_general_model, GeneralModel};
use trust_control::runtime::RuntimeConnection;
use trust_control_studio::loops;
use trust_control_studio::runtime_client::parameters::{runtime_binding, ParameterTransactions};
use trust_control_studio::runtime_client::transfers::{
    TransferOwnerSink, TransferSession, TransferWrite,
};
use trust_control_studio::runtime_client::RuntimeClient;

use common::{
    control_socket, copy_dir, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, TOKEN,
};

/// The lesson's project, a copy of which the test runs.
const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The output span of the lesson's controller, 0 to 100 %: the output change that a transfer allows at its first execution, which
/// the engineer may set anywhere from zero to the span (annex E6.9).
const OUTPUT_SPAN: f64 = 100.0;

/// The heater's model of the lesson (`Main.st`: a gain of 0.2 °C per percent, a time constant of 5 s and a delay of 1 s), at its operating
/// point of 75 % and 35 °C. The declared profile is built from a model of this kind (A6.4).
fn lesson_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "revision": "model:lesson-30",
        "source": "provided",
        "validation": "validated",
        "evidence": ["lesson-30"],
        "delaySeconds": 1.0,
        "numerator": [0.2],
        "denominator": [5.0, 1.0],
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "degC", "operatingRange": { "minimum": 0, "maximum": 80 } }],
        "operatingPoint": { "inputs": [75], "outputs": [35] }
    }))
    .expect("the lesson's model decodes")
}

/// The values of the binding's states in a transfer status: every state is a number, and a mode is not one of them.
fn state_values(status: &TransferStatus, binding: &TransferBinding) -> BTreeMap<String, f64> {
    binding
        .states
        .iter()
        .map(|member| {
            let value = status
                .values
                .get(&member.symbol)
                .and_then(|value| value.as_number())
                .expect("the runtime reports each state as a number");
            (member.symbol.clone(), value)
        })
        .collect()
}

/// The first parameter moved by 1 % of its span, toward the middle of its bounds; every other parameter keeps its value. The record must
/// differ from the one in force, because a record that is the same leaves the revision where it was.
fn candidate_values(
    original: &BTreeMap<String, f64>,
    binding: &ParameterBinding,
) -> BTreeMap<String, f64> {
    binding
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let value = original
                .get(&parameter.symbol)
                .copied()
                .expect("the runtime reports each declared parameter");
            let delta = (parameter.maximum - parameter.minimum) * 0.01;
            let candidate = match index {
                0 if value + delta <= parameter.maximum => value + delta,
                0 => value - delta,
                _ => value,
            };
            (parameter.symbol.clone(), candidate)
        })
        .collect()
}

/// The owner sink of the journey: each owner is kept in memory, which is where the engine's journal keeps it before the apply (E11.2).
#[derive(Default)]
struct Journal(Vec<TransferOwner>);

impl TransferOwnerSink for Journal {
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure> {
        self.0.push(owner.clone());
        Ok(())
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One journey against the runtime, step by step; split, it would hide the order of the transactions (E11.2).
fn the_transactions_of_lesson_30_are_confirmed_by_the_runtime() {
    let folder =
        std::env::temp_dir().join(format!("trust-cs-v4-transactions-{}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    let _remove = RemoveDir(folder.clone());
    let project = folder.join("project");
    copy_dir(Path::new(EXAMPLE), &project);
    let socket = control_socket(&project);
    prepare_runtime_toml(&project, &socket);
    run_trust_runtime(&["build", "--project", &project.display().to_string()]);

    let text = fs::read_to_string(project.join("control-loop.json")).expect("the lesson's loop");
    let declared = LoopDefinition::decode(&text).expect("the lesson's loop decodes");
    let loop_definition = loops::open(&project, &declared.runtime_id, &declared.id)
        .expect("Control Studio opens the loop");
    let parameter_binding = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the lesson declares its parameter binding");

    let log = project.join("runtime.log");
    let _runtime = start_runtime(&project, &log);
    let connection = RuntimeConnection {
        endpoint: format!("unix://{}", socket.display()),
        token: Some(TOKEN.into()),
        runtime_id: None,
    };
    let first = wait_for_status(&connection, &log);
    let mut client = RuntimeClient::connect(&connection).expect("connected");
    let program = first.program_revision.clone();

    // The declared controller: its transfer binding, its recovery tracking, and its profile for the Auto entry.
    let adapter = controller_adapter(&loop_definition).expect("the controller has an adapter");
    let binding = adapter.transfer_binding().expect("the transfer binding");
    let model = lesson_model();
    let recovery = adapter
        .recovery_tracking(&model)
        .expect("the recovery tracking");
    // The program initializes its controller in its first execution (`IF NOT Initialized`), and designs its gains there; a
    // transfer is reviewed on the running program, after that (B6.2), and the record it replaces is the one the program set.
    wait_until_started(&mut client, &binding);
    let original = client
        .parameter_values(&loop_definition)
        .expect("the runtime reports the parameter values");

    // Manual: the loop is transferred to Manual with its recovery tracking, and the runtime confirms it.
    {
        let mut session = TransferSession::new(&mut client, binding.clone(), program.clone());
        let observed = session.status().expect("the transfer status");
        let mut journal = Journal::default();
        let outcome = session
            .apply(
                &TransferWrite {
                    values: state_values(&observed, &binding),
                    manual: true,
                    tolerance: OUTPUT_SPAN,
                    entry_error_gain: None,
                    tracking: Some(recovery.clone()),
                    reviewed: None,
                    source: ApplySource::Simulation,
                    physical_approved: false,
                },
                &mut journal,
            )
            .expect("the loop is transferred to Manual");
        assert_eq!(outcome.status, TransferState::Applied);
        assert_eq!(journal.0.len(), 1, "the owner is retained before the apply");
        assert!(
            session.owned().is_none(),
            "a confirmed transfer releases its owner"
        );
    }

    // Manual: a parameter record is applied and confirmed, and then rolled back to the values it replaced.
    {
        let mut transactions = ParameterTransactions::new(
            &mut client,
            runtime_binding(parameter_binding),
            ApplySource::Simulation,
        );
        let candidate = candidate_values(&original, parameter_binding);
        let review = transactions
            .review(&candidate)
            .expect("the record is reviewed against the runtime");
        let applied = transactions
            .apply(&review, false)
            .expect("the record is applied in Manual");
        assert_eq!(applied.status, RecordState::Applied);
        assert_eq!(applied.values, candidate);

        let rollback = transactions
            .review_rollback()
            .expect("the replaced values are retained");
        assert_eq!(rollback.values, original);
        let restored = transactions
            .rollback(&rollback, false)
            .expect("the replaced values are restored");
        assert_eq!(restored.status, RecordState::Applied);
        assert_eq!(restored.values, original);
    }

    // Auto: the loop is transferred back with the tracking of its profile, and the runtime confirms the output image.
    {
        let profile = adapter
            .declared_profile(&model, &original)
            .expect("the declared profile fits the loop")
            .expect("Control.PID declares its profile");
        let mut session = TransferSession::new(&mut client, binding.clone(), program.clone());
        let observed = session.status().expect("the transfer status");
        let states = profile
            .states(&binding, &observed.values)
            .expect("the declared states");
        let tracking = profile
            .tracking(&binding, true)
            .expect("the declared tracking");
        let mut journal = Journal::default();
        let outcome = session
            .apply(
                &TransferWrite {
                    values: states,
                    manual: false,
                    tolerance: OUTPUT_SPAN,
                    entry_error_gain: None,
                    tracking: Some(tracking),
                    reviewed: None,
                    source: ApplySource::Simulation,
                    physical_approved: false,
                },
                &mut journal,
            )
            .expect("the loop is transferred back to Auto");
        assert_eq!(outcome.status, TransferState::Applied);
        assert_eq!(journal.0.len(), 1);
    }

    // An identifier that the runtime does not hold is cancelled before dispatch, and the cancel is confirmed (annex E0.2, E6.10).
    let answer = client
        .transfer_cancel(&TransferCancelRequest {
            loop_id: binding.loop_id.clone(),
            transaction_id: "transaction-never-dispatched".to_owned(),
            expected_program_revision: program,
        })
        .expect("the runtime answers the cancel");
    assert!(
        answer.confirmed && answer.before_dispatch,
        "an unknown identifier is cancelled before dispatch: {answer:?}"
    );
    assert_eq!(answer.status, Some(TransferState::Cancelled));
    assert_eq!(
        answer.transaction_id.as_deref(),
        Some("transaction-never-dispatched")
    );
}

/// Waits until the program has run its startup: its `Initialized` flag reads TRUE in the transfer status.
fn wait_until_started(client: &mut RuntimeClient, binding: &TransferBinding) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let status = client
            .transfer_status(binding)
            .expect("the transfer status");
        let started = matches!(
            status.values.get("P1.Initialized"),
            Some(TransferValue::Boolean(true))
        ) || matches!(
            status.values.get("P1.Initialized"),
            Some(TransferValue::Number(value)) if *value >= 0.5
        );
        if started {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the program did not run its startup within 20 s"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
