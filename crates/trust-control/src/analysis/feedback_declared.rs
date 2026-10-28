//! The general feedback loop of a declared controller law (N7.6, N7.7 to N7.11): the law's response on the plant's grid,
//! the closed-loop characteristic polynomial and the Nyquist count of the declared loop, and the feedback definition
//! that a declared analysis gives a plant. The shared loop is in `general_feedback`.

// A declared denominator is zero or not, and a sample time is compared exactly, as N7.6 states the rules.
#![allow(clippy::float_cmp)]

use std::f64::consts::PI;

use trust_control_math::classical::{analyze_transfer, roots_of_real_polynomial, Transfer};
use trust_control_math::linalg::ComplexValue;
use trust_control_math::model::TransferFunction;
use trust_control_math::polynomial::{add_aligned, convolve};

use crate::analysis::declared::DeclaredControlAnalysis;
use crate::analysis::feedback_margins::{
    closed_loop_result, contour_count_for, estimate_margins, report,
};
use crate::analysis::frequency_points::grid_of;
use crate::analysis::general_feedback::{
    finish, linearised, multiply_points, open_loop_points, sensitivity_points, ChannelPair,
    ClosedLoop, ControllerSource, DeclaredFeedbackController, FeedbackDefinition, FeedbackRefusal,
    FeedbackResult, NyquistReport,
};
use crate::analysis::texts;
use crate::model::{GeneralModel, ModelChannel, ModelIdentity};

/// The loop of a declared controller law: the plant's channel times the law at each frequency, the closed-loop
/// poles of the declared characteristic polynomial, and the Nyquist count of the declared loop.
#[allow(clippy::too_many_lines)] // The law's response, its closure and its count, in the order that N7.6 and N7.7 state.
pub(crate) fn analyze_declared(
    definition: &FeedbackDefinition,
    declared: &DeclaredFeedbackController,
    frequencies: &[f64],
    plant_points: &[ComplexValue],
) -> Result<FeedbackResult, FeedbackRefusal> {
    let plant = match &definition.plant {
        GeneralModel::ContinuousStateSpace { .. } => linearised(&definition.plant)?,
        GeneralModel::TransferFunction { identity, transfer } => Transfer {
            numerator: transfer.numerator.clone(),
            denominator: transfer.denominator.clone(),
            delay_seconds: identity.delay_seconds,
        },
        GeneralModel::DiscreteStateSpace { .. } => {
            return Err(FeedbackRefusal::Other(
                texts::DECLARED_PLANT_UNSUPPORTED.to_string(),
            ));
        }
    };
    let law = Transfer {
        numerator: declared.numerator.clone(),
        denominator: declared.denominator.clone(),
        delay_seconds: 0.0,
    };
    let response = analyze_transfer(&law, frequencies)
        .map_err(|error| FeedbackRefusal::Other(texts::classical_reason(error).to_string()))?;
    if plant_points.len() != response.bode.len() {
        return Err(FeedbackRefusal::Other(
            texts::DECLARED_GRID_MISMATCH.to_string(),
        ));
    }
    let controller_points: Vec<ComplexValue> = response
        .bode
        .iter()
        .map(|point| {
            let magnitude = 10f64.powf(point.magnitude_db / 20.0);
            let phase = point.phase_degrees * PI / 180.0;
            ComplexValue {
                real: magnitude * phase.cos(),
                imaginary: magnitude * phase.sin(),
            }
        })
        .collect();
    let open_loop = multiply_points(plant_points, &controller_points, frequencies)?;
    let sensitivity = sensitivity_points(&open_loop, &controller_points, frequencies);
    let characteristic = add_aligned(
        &convolve(&plant.denominator, &law.denominator),
        &convolve(&plant.numerator, &law.numerator),
    );
    let closed_loop = if plant.delay_seconds > 0.0 {
        ClosedLoop::Unavailable {
            reason: texts::FEEDBACK_DELAY_POLES.to_string(),
        }
    } else {
        let poles = roots_of_real_polynomial(&characteristic).ok_or_else(|| {
            FeedbackRefusal::Other(texts::DECLARED_CHARACTERISTIC_UNSOLVED.to_string())
        })?;
        let scope = format!(
            "{}{}{}",
            texts::FEEDBACK_DECLARED_POLES_SCOPE,
            if matches!(definition.plant, GeneralModel::ContinuousStateSpace { .. }) {
                texts::FEEDBACK_DECLARED_STATE_SUFFIX
            } else {
                ""
            },
            texts::FEEDBACK_DECLARED_POLES_TAIL
        );
        closed_loop_result(poles, &definition.plant, scope)
    };
    let nyquist = report(contour_count_for(
        &Transfer {
            numerator: convolve(&plant.numerator, &law.numerator),
            denominator: convolve(&plant.denominator, &law.denominator),
            delay_seconds: plant.delay_seconds,
        },
        frequencies,
    ));
    let nyquist_scope = match &nyquist {
        NyquistReport::Qualified { .. } => texts::FEEDBACK_NYQUIST_DECLARED_SCOPE.to_string(),
        NyquistReport::Rejected { reason } => {
            format!("The shared Nyquist stability owner rejected the requested contour: {reason}")
        }
    };
    Ok(finish(FeedbackResult {
        open_loop: open_loop_points(&open_loop, frequencies),
        nyquist_scope,
        sensitivity: Some(sensitivity),
        nyquist: Some(nyquist),
        closed_loop,
        margins: estimate_margins(definition, frequencies),
        grid: grid_of(frequencies),
        plant_channel: definition.plant_channel.clone(),
        controller_channel: definition.controller_channel.clone(),
        controller_source: ControllerSource::DeclaredLaw,
        summary: Vec::new(),
        details: Vec::new(),
    }))
}

/// The declared controller law as a feedback definition on the plant's channels, or `None` when the declared
/// law does not fit the plant: a discrete or non-SISO plant, unit mismatches, or a law that does not evaluate
/// (N7.6). The controller model is a placeholder that carries the channels and the units.
#[must_use]
pub fn declared_feedback_definition(
    analysis: &DeclaredControlAnalysis,
    plant: &GeneralModel,
) -> Option<FeedbackDefinition> {
    crate::analysis::declared::check_declared(analysis).ok()?;
    if plant.identity().inputs.len() != 1 || plant.identity().outputs.len() != 1 {
        return None;
    }
    let (input, output) = (&plant.identity().inputs[0], &plant.identity().outputs[0]);
    match plant {
        GeneralModel::DiscreteStateSpace { .. } => return None,
        GeneralModel::ContinuousStateSpace { .. } => {
            linearised(plant).ok()?;
        }
        GeneralModel::TransferFunction { .. } => {}
    }
    if analysis.plant.input_unit != input.unit || analysis.plant.output_unit != output.unit {
        return None;
    }
    let sweep = &analysis.root_locus;
    let controller = &analysis.controller;
    let evaluate = |coefficients: &[crate::analysis::declared::DeclaredCoefficient]| -> Vec<f64> {
        crate::analysis::declared::substitute(
            coefficients,
            &sweep.parameter,
            sweep.selected_value,
            controller,
        )
    };
    let numerator = evaluate(&controller.numerator);
    let denominator = evaluate(&controller.denominator);
    if !numerator.iter().all(|value| value.is_finite())
        || !denominator.iter().all(|value| value.is_finite())
        || denominator[0] == 0.0
    {
        return None;
    }
    let revision = analysis.plant.revision.clone();
    let placeholder_identity = ModelIdentity {
        revision: format!("{revision}:declared-controller-analysis"),
        source: analysis.plant.source,
        validation: analysis.plant.validation,
        evidence: vec![format!("declared-controller-law:{revision}")],
        inputs: vec![ModelChannel {
            id: format!("{}:feedback", output.id),
            unit: output.unit.clone(),
            operating_range: output.operating_range,
        }],
        outputs: vec![ModelChannel {
            id: format!("{}:command", input.id),
            unit: input.unit.clone(),
            operating_range: input.operating_range,
        }],
        delay_seconds: 0.0,
        operating_point: None,
    };
    let controller_model = GeneralModel::TransferFunction {
        identity: placeholder_identity,
        transfer: TransferFunction {
            numerator: vec![1.0],
            denominator: vec![1.0],
        },
    };
    let controller_channel = ChannelPair {
        input_id: format!("{}:feedback", output.id),
        output_id: format!("{}:command", input.id),
    };
    Some(FeedbackDefinition {
        plant: plant.clone(),
        controller: controller_model,
        plant_channel: ChannelPair {
            input_id: input.id.clone(),
            output_id: output.id.clone(),
        },
        controller_channel,
        declared_controller: Some(DeclaredFeedbackController {
            numerator,
            denominator,
            parameter: sweep.parameter.clone(),
            selected_value: sweep.selected_value,
            revision,
            input_unit: output.unit.clone(),
            output_unit: input.unit.clone(),
        }),
    })
}
