//! Lesson 30's heater (`examples/control/06-pid-tuning-methods/30-model-based-pi-tuning`) as the real-runtime journeys drive it: its
//! model at the operating point, the transfer between Manual and Auto that the engine's own transfers make, and the wait for the
//! water to return to the operating point before Apply (specification 35, A6.4, B6.2; annex E6). Moved from
//! `control_studio_apply.rs`, which the journey of `control_studio_gvl_loop.rs` shares.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use trust_control::commissioning::adapters::controller_adapter;
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::commissioning_result::{
    TransferOwner, TransferState, TransferStatus,
};
use trust_control::commissioning::transfer::{TransferBinding, TransferValue};
use trust_control::failure::Failure;
use trust_control::model::general::{decode_general_model, GeneralModel};
use trust_control_studio::runtime_client::transfers::{
    TransferOwnerSink, TransferSession, TransferWrite,
};

use super::engine::Lab;

/// Lesson 30's heater project, a copy of which a journey runs.
pub const LESSON_30: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The longest that one verb may run; at ten times real time the verbs take seconds, so this only ends a hung run.
pub const LIMIT: Duration = Duration::from_secs(300);
/// The output span of the heater, 0 to 100 %: the change that a transfer may make at its first execution (annex E6.9).
pub const OUTPUT_SPAN: f64 = 100.0;
/// The heater's operating point, where Identify found the plant and Validate checked the candidate: 35 °C at 75 % (B6.2).
pub const OPERATING_POINT: f64 = 35.0;
/// How close to the operating point the water must be before the loop enters Auto: a third of the study's settling band of ± 0.03 °C,
/// so that the setpoint the entry aligns to the water (annex E6.4) lies well inside the band that Apply waits for (B6.2).
pub const RETURNED_WITHIN: f64 = 0.01;

/// The heater's model of the lesson (`Main.st`: a gain of 0.2 °C per percent, a time constant of 5 s and a delay of 1 s), at its
/// operating point of 75 % and 35 °C. The declared profile is built from a model of this kind (A6.4).
pub fn lesson_model() -> GeneralModel {
    decode_general_model(&serde_json::json!({
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

/// The values of the binding's states in a transfer status: each one is a number.
pub fn state_values(status: &TransferStatus, binding: &TransferBinding) -> BTreeMap<String, f64> {
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

/// The owner sink of a transfer of this journey: each owner is kept in memory, where the engine's journal keeps it before the dispatch
/// (E11.2).
#[derive(Default)]
pub struct OwnerSink(Vec<TransferOwner>);

impl TransferOwnerSink for OwnerSink {
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure> {
        self.0.push(owner.clone());
        Ok(())
    }
}

/// Moves the loop to Manual (`manual`) or to Auto through the runtime's transfer, as the engine's own transfers do: a move to Manual
/// tracks the controller's recovery tracking, a move to Auto enters with the declared profile of the controller (A6.4). The runtime
/// confirms the transfer, and the test checks that it did.
pub fn transfer(lab: &Lab, manual: bool) {
    let adapter = controller_adapter(&lab.loop_definition).expect("the controller has an adapter");
    let binding = adapter.transfer_binding().expect("the transfer binding");
    let model = lesson_model();
    let original = lab.parameter_values();
    let mut client = lab.client();
    let mut session =
        TransferSession::new(&mut client, binding.clone(), lab.program_revision.clone());
    let observed = session.status().expect("the transfer status");
    let (values, tracking) = if manual {
        let recovery = adapter
            .recovery_tracking(&model)
            .expect("the recovery tracking");
        (state_values(&observed, &binding), recovery)
    } else {
        let profile = adapter
            .declared_profile(&model, &original)
            .expect("the declared profile fits the loop")
            .expect("Control.PID declares its profile");
        let states = profile
            .states(&binding, &observed.values)
            .expect("the declared states");
        let tracking = profile
            .tracking(&binding, true)
            .expect("the declared tracking");
        (states, tracking)
    };
    let mut sink = OwnerSink::default();
    let outcome = session
        .apply(
            &TransferWrite {
                values,
                manual,
                tolerance: OUTPUT_SPAN,
                entry_error_gain: None,
                entry_measurement_gain: None,
                tracking: Some(tracking),
                reviewed: None,
                source: ApplySource::Simulation,
                physical_approved: false,
            },
            &mut sink,
        )
        .expect("the runtime answers the transfer");
    assert_eq!(
        outcome.status,
        TransferState::Applied,
        "the runtime confirms the transfer (E6.7): {:?}",
        outcome.reason
    );
    assert_eq!(sink.0.len(), 1, "the owner is retained before the dispatch");
}

/// The loop's reference as the runtime's transfer status reports it (annex E6.3), or `None` when it cannot be read.
pub fn reference(lab: &Lab) -> Option<f64> {
    let adapter = controller_adapter(&lab.loop_definition).ok()?;
    let binding = adapter.transfer_binding().ok()?;
    let symbol = lab
        .loop_definition
        .signals
        .setpoint
        .as_ref()?
        .symbol
        .clone();
    let mut client = lab.client();
    let mut session = TransferSession::new(&mut client, binding, lab.program_revision.clone());
    session
        .status()
        .ok()?
        .values
        .get(&symbol)
        .copied()
        .and_then(TransferValue::as_number)
}

/// The loop's measurement as the runtime's transfer status reports it (annex E6.3), or `None` when it cannot be read.
pub fn measurement(lab: &Lab) -> Option<f64> {
    let adapter = controller_adapter(&lab.loop_definition).ok()?;
    let binding = adapter.transfer_binding().ok()?;
    let symbol = lab.loop_definition.signals.process_variable.symbol.clone();
    let mut client = lab.client();
    let mut session = TransferSession::new(&mut client, binding, lab.program_revision.clone());
    session
        .status()
        .ok()?
        .values
        .get(&symbol)
        .copied()
        .and_then(TransferValue::as_number)
}

/// Waits until the water is back at the operating point after the plant steps of Identify and of Validate's model check, as an
/// engineer waits before Apply (B6.2), and returns the last measurement. An Auto entry aligns the setpoint to the water (annex E6.4),
/// so an entry while the water still returns holds the loop away from the operating point, where Apply rightly refuses to start. The
/// water needs about 28 s of process time after the model check's step to come within 0.01 °C; at ten times real time the journey
/// gives it that before the entry, but a runner that reaches two or three times real time (macOS CI, 28 October 2026) did not.
pub fn wait_for_operating_point(lab: &Lab) -> Option<f64> {
    let deadline = Instant::now() + LIMIT;
    loop {
        let value = measurement(lab);
        if value.is_some_and(|value| (value - OPERATING_POINT).abs() <= RETURNED_WITHIN)
            || Instant::now() >= deadline
        {
            return value;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
