//! Tests of the plant model that identification hands to Control Studio (`selectedModelAnalysis.ts`).

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.
#![allow(clippy::float_cmp)] // The coefficients and delays are exact products of exact inputs.

use super::{provisional_identification_model, ChannelSet, OperatingValues};
use crate::identification::data::IdentificationSample;
use crate::identification::plant::IdentifiedModel;
use crate::model::{GeneralModel, ModelChannel, ModelSource, ModelValidation, OperatingRange};
use trust_control_math::identify::LowOrderModelKind;

fn model(kind: LowOrderModelKind, gain: f64) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: None,
        normalized_error: 0.01,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

fn channels() -> ChannelSet {
    let channel = |id: &str, unit: &str| ModelChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
        operating_range: OperatingRange {
            minimum: 0.0,
            maximum: 100.0,
        },
    };
    ChannelSet {
        inputs: vec![channel("MV", "%")],
        outputs: vec![channel("PV", "degC")],
    }
}

/// The transfer function and the operating point of a provisional model of `identified`, with the operating values given.
fn transfer(identified: &IdentifiedModel) -> GeneralModel {
    provisional_identification_model(
        identified,
        &[] as &[IdentificationSample],
        &channels(),
        "sha256:record",
        Some(OperatingValues {
            input0: 1.0,
            output0: 4.0,
        }),
    )
    .expect("a provisional model of one input and one output")
}

#[test]
fn a_first_order_fit_becomes_a_fitted_unvalidated_transfer_function() {
    let mut identified = model(LowOrderModelKind::FirstOrder, 2.0);
    identified.time_constant_seconds = Some(3.0);
    let GeneralModel::TransferFunction {
        identity,
        transfer: polynomials,
    } = transfer(&identified)
    else {
        panic!("a low-order model is a transfer function");
    };
    assert_eq!(identity.source, ModelSource::Fitted);
    assert_eq!(identity.validation, ModelValidation::Unvalidated);
    assert_eq!(identity.evidence, vec!["sha256:record".to_owned()]);
    assert_eq!(
        polynomials.numerator,
        vec![2.0],
        "the numerator is the gain"
    );
    assert_eq!(
        polynomials.denominator,
        vec![3.0, 1.0],
        "τs + 1 in descending powers"
    );
    assert_eq!(identity.delay_seconds, 0.0, "no dead time is no delay");
    let point = identity
        .operating_point
        .expect("the operating point is kept");
    assert_eq!(point.inputs, vec![1.0]);
    assert_eq!(point.outputs, vec![4.0]);
    assert_eq!(identity.inputs[0].id, "MV");
    assert_eq!(identity.outputs[0].unit, "degC");
    assert!(identity.revision.starts_with("sha256:"));
}

#[test]
fn an_integrating_model_multiplies_by_the_integrator_and_its_time_constant() {
    let mut identified = model(LowOrderModelKind::Integrating, 0.5);
    identified.time_constant_seconds = Some(2.0);
    let GeneralModel::TransferFunction {
        transfer: polynomials,
        ..
    } = transfer(&identified)
    else {
        panic!("a low-order model is a transfer function");
    };
    // s(2s + 1) = 2s² + s
    assert_eq!(polynomials.denominator, vec![2.0, 1.0, 0.0]);
}

#[test]
fn a_second_order_fit_multiplies_both_time_constants_and_keeps_the_dead_time() {
    let mut identified = model(LowOrderModelKind::SecondOrderDeadTime, 1.0);
    identified.time_constant_seconds = Some(1.0);
    identified.second_time_constant_seconds = Some(2.0);
    identified.dead_time_seconds = Some(0.5);
    let GeneralModel::TransferFunction {
        identity,
        transfer: polynomials,
    } = transfer(&identified)
    else {
        panic!("a low-order model is a transfer function");
    };
    // (s + 1)(2s + 1) = 2s² + 3s + 1
    assert_eq!(polynomials.denominator, vec![2.0, 3.0, 1.0]);
    assert_eq!(identity.delay_seconds, 0.5);
}

#[test]
fn the_revision_changes_with_the_model_and_channels_are_one_in_one_out() {
    let first = transfer(&model(LowOrderModelKind::StaticGain, 2.0));
    let second = transfer(&model(LowOrderModelKind::StaticGain, 2.5));
    assert_ne!(first.identity().revision, second.identity().revision);
    let two_inputs = ChannelSet {
        inputs: vec![channels().inputs[0].clone(), channels().inputs[0].clone()],
        outputs: channels().outputs,
    };
    let refused = provisional_identification_model(
        &model(LowOrderModelKind::StaticGain, 2.0),
        &[],
        &two_inputs,
        "sha256:record",
        Some(OperatingValues {
            input0: 0.0,
            output0: 0.0,
        }),
    );
    assert_eq!(
        refused.err().map(|refusal| refusal.0),
        Some("Low-order identification requires one input and one output channel.".to_owned())
    );
}

#[test]
fn the_estimation_operating_point_is_kept_through_acceptance() {
    use crate::identification::data::{DataSource, IdentificationSample};
    use crate::identification::review::IdentificationModelSelection;
    use crate::identification::workflow::{IdentificationWorkflow, QualifyRequest};
    use trust_control_math::identify::SampleQuality;

    // A first-order step on 2 V from an output of 10 m, then 0.75 V from 11 m: the accepted model keeps the estimation
    // point (2 V, 10 m), not the validation's.
    let samples = |amplitude: f64, output0: f64| -> Vec<IdentificationSample> {
        (0..301_usize)
            .map(|index| {
                let step = index >= 20;
                let elapsed = index as f64 - 20.0;
                IdentificationSample {
                    time_seconds: index as f64 * 0.1,
                    input: 2.0 + if step { amplitude } else { 0.0 },
                    output: output0
                        + if step {
                            amplitude * (1.0 - (-elapsed * 0.1).exp())
                        } else {
                            0.0
                        },
                    quality: SampleQuality::Good,
                    input_unit: Some("V".to_owned()),
                    output_unit: Some("m".to_owned()),
                    clipped: None,
                    saturated: None,
                }
            })
            .collect()
    };
    let request = |samples: Vec<IdentificationSample>, id: &str| QualifyRequest {
        samples,
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind: IdentificationModelSelection::FirstOrder,
        recording_id: Some(id.to_owned()),
        step_response_settings: None,
        recording: None,
        first_order_step_response_settings: None,
    };
    let mut workflow = IdentificationWorkflow::new(0.15);
    workflow.qualify(request(samples(1.0, 10.0), "estimation"));
    workflow.fit().expect("the estimation fits");
    workflow.qualify(request(samples(0.75, 11.0), "validation"));
    workflow.fit().expect("the validation fits");
    let accepted = workflow
        .accept()
        .expect("the independent records accept the model");
    let channel = |id: &str, unit: &str| ModelChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
        operating_range: OperatingRange {
            minimum: 0.0,
            maximum: 100.0,
        },
    };
    let selected = super::accepted_identification_model(
        &accepted,
        &ChannelSet {
            inputs: vec![channel("u", "V")],
            outputs: vec![channel("y", "m")],
        },
    )
    .expect("the accepted model is a transfer function");
    let point = selected
        .identity()
        .operating_point
        .clone()
        .expect("the operating point is kept");
    assert_eq!(point.inputs, vec![2.0]);
    assert_eq!(point.outputs, vec![10.0]);
}
