//! Tune of a trajectory-following law (specification 35, C11): the PID is designed as for Control.PID, and the feedforward gains come from
//! the model's transfer function. A model of another form has no trajectory feedforward rule, so Tune stores no candidate and says why.

use trust_control_math::model::OperatingPoint;

use super::*;
use crate::experiment::ControlExperimentKind;
use crate::identification::identify_result::identify_signals;
use crate::messages::{TuneMethod, TuneRequest};
use crate::model::GeneralModel;
use crate::study::identify::IdentifySection;
use crate::study::tune::model::tests::identify_section;
use crate::study::tune::tune;
use crate::tuning::texts;

/// Lesson 90 as the shipped file declares it: the drum axis, served by its program's trajectory-feedforward PID.
fn lesson_90() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../../examples/control/09-industrial-control-structures/90-trajectory-feedforward/control-loop.json"
    ))
    .expect("lesson 90 decodes")
}

/// The request of a Tune run on lesson 90 with the method given, the PI structure fixed, as the program-PI tests of the study fix theirs.
fn request(method: TuneMethod) -> TuneRequest {
    TuneRequest {
        project_root: "/projects/control-trajectory-feedforward".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-trajectory-feedforward".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method: Some(method),
        choice: None,
        structure: Some(Structure::PI),
        pins: None,
        settings: None,
        connection: None,
    }
}

#[test]
fn a_first_order_model_has_no_trajectory_feedforward_so_tune_refuses_with_the_sentence() {
    // The study's model is the first-order model of the model tests, operating inside lesson 90's own limits (the torque within its
    // ±20 N·m and the angle within its ±2 rad), so that Tune's operating-limit checks pass: the PID is designed, and the feedforward
    // is refused.
    let loop_definition = lesson_90();
    let base = identify_section(None);
    let mut selected_model = base.selected_model.clone();
    let GeneralModel::TransferFunction { identity, .. } = &mut selected_model else {
        panic!("the first-order fixture is a transfer function");
    };
    identity.operating_point = Some(OperatingPoint {
        inputs: vec![10.0],
        outputs: vec![1.0],
    });
    let section = IdentifySection::new(
        base.method,
        base.source,
        identify_signals(&loop_definition, ControlExperimentKind::SetpointStep),
        &base.estimation_recording_id,
        None,
        selected_model,
        base.presentation
            .clone()
            .expect("a fit keeps its presentation"),
    );
    let current = CurrentModel::identified(section);
    let failure = tune(&TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &request(TuneMethod::Simc),
        relay: None,
        pins: None,
    })
    .expect_err("a first-order model is not n/(a·s² + b·s)");
    assert_eq!(failure.message, texts::TRAJECTORY_REFUSED);
}
