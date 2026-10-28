//! Tests of the production path support (specification 35, C6 to C9): the synthetic model of a reduced path, its predictor and its
//! placement, and the PI that the engine designs on a path with it. The forms are read back exactly; the predictor starts at the
//! operating output; the placement and the design are checked for their signs, as the rules of N5.12 give them.

#![allow(clippy::float_cmp)] // The forms are read back exactly from the path, which the tests compare with `==`.

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::tuning::{FirstOrderPlusDelay, IntegratingModel, SecondOrderPlusDelay};

use super::*;
use crate::tuning::shortfall::GainLimits;
use crate::tuning::structures::fixtures::requirements;
use crate::tuning::structures::path::design_pi_on_path;

/// A first-order path with the gain, the time constant and the dead time given.
fn first_order(gain: f64, time_constant_seconds: f64, delay_seconds: f64) -> TuningPlant {
    TuningPlant::FirstOrder(FirstOrderPlusDelay {
        gain,
        time_constant_seconds,
        delay_seconds,
    })
}

/// The facts of a heater loop: direct action, a 100 ms task, output from 0 to 100 %, rates that are not declared, and no record
/// shaping.
fn facts() -> LoopFacts {
    LoopFacts {
        action: ControllerAction::Direct,
        task_period_ms: 100.0,
        gain_limits: GainLimits::default(),
        output_limits: OutputLimits {
            minimum: 0.0,
            maximum: 100.0,
        },
        rate_limits: ParameterInterval {
            minimum: -1e6,
            maximum: 1e6,
        },
        shaping: RecordShaping::default(),
        process_unit: "degC".to_owned(),
        manipulated_unit: "%".to_owned(),
    }
}

#[test]
fn a_first_order_path_is_a_first_order_model_with_its_dead_time() {
    let model = path_model(&first_order(0.2, 8.0, 1.5)).expect("a first-order path has a model");
    assert_eq!(model.kind, LowOrderModelKind::FirstOrderDeadTime);
    assert_eq!(model.gain, 0.2);
    assert_eq!(model.time_constant_seconds, Some(8.0));
    assert_eq!(model.second_time_constant_seconds, None);
    assert_eq!(model.dead_time_seconds, Some(1.5));
    assert!(model.stable, "a reduced path is stable");
}

#[test]
fn a_first_order_path_without_a_delay_is_a_first_order_model() {
    let model = path_model(&first_order(-0.1, 3.0, 0.0)).expect("a first-order path has a model");
    assert_eq!(model.kind, LowOrderModelKind::FirstOrder);
    assert_eq!(model.gain, -0.1);
    assert_eq!(model.dead_time_seconds, Some(0.0));
}

#[test]
fn an_integrating_path_is_an_integrating_model_with_its_rate_gain() {
    let path = TuningPlant::Integrating(IntegratingModel {
        rate_gain: 0.5,
        delay_seconds: 2.0,
    });
    let model = path_model(&path).expect("an integrating path has a model");
    assert_eq!(model.kind, LowOrderModelKind::Integrating);
    assert_eq!(model.gain, 0.5);
    assert_eq!(model.time_constant_seconds, None);
    assert_eq!(model.dead_time_seconds, Some(2.0));
}

#[test]
fn a_second_order_path_has_no_synthetic_model() {
    let path = TuningPlant::SecondOrder(SecondOrderPlusDelay {
        gain: 1.0,
        leading: 2.0,
        linear: 3.0,
        constant: 1.0,
        delay_seconds: 0.0,
    });
    assert_eq!(path_model(&path), None);
    let predictor = ReducedPathSupport.predictor(&path, &facts(), (80.0, 35.0));
    let design = TypedDesign {
        structure: Structure::PI,
        gains: ParallelGains {
            proportional_gain: 1.0,
            integral_gain: 0.1,
            derivative_gain: 0.0,
        },
    };
    let test = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    assert!(
        predictor
            .predict(&design, Trial::SetpointChange, &test)
            .is_none(),
        "a path without a model is not predicted"
    );
}

#[test]
fn the_support_predicts_a_setpoint_response_on_the_path_from_its_operating_output() {
    let path = first_order(0.2, 8.0, 0.0);
    let predictor = ReducedPathSupport.predictor(&path, &facts(), (80.0, 35.0));
    let design = TypedDesign {
        structure: Structure::PI,
        gains: ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.1,
            derivative_gain: 0.0,
        },
    };
    let test = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    let points = predictor
        .predict(&design, Trial::SetpointChange, &test)
        .expect("the path is predicted");
    assert!(points.len() > 1, "the prediction covers the test");
    assert!(
        (points[0].process_value - 35.0).abs() < 1e-6,
        "the response starts at the operating output: {}",
        points[0].process_value
    );
}

#[test]
fn the_support_places_a_pi_on_a_first_order_path() {
    let path = first_order(0.2, 8.0, 0.0);
    let test = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    let predictor = ReducedPathSupport.predictor(&path, &facts(), (80.0, 35.0));
    let context = DesignContext {
        action: ControllerAction::Direct,
        task_period_ms: 100.0,
        operating_input: 80.0,
        operating_output: 35.0,
        output_limits: OutputLimits {
            minimum: 0.0,
            maximum: 100.0,
        },
        gain_limits: GainLimits::default(),
        process_unit: "degC",
        manipulated_unit: "%",
        predictor: predictor.as_ref(),
    };
    let placement = ReducedPathSupport.placement(&path, &test, &context);
    let gains = placement
        .gains(Structure::PI, 4.0, 40.0)
        .expect("a first-order path has a PI placement");
    assert!(
        gains.proportional_gain > 0.0 && gains.integral_gain > 0.0,
        "{gains:?}"
    );
}

#[test]
fn the_pi_on_a_path_is_designed_by_the_engine_with_no_derivative_term() {
    let path = first_order(0.2, 8.0, 0.0);
    let test = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    let gains = design_pi_on_path(&path, &facts(), (80.0, 35.0), &test, &ReducedPathSupport)
        .expect("the path's PI is designed");
    assert!(
        gains.proportional_gain > 0.0 && gains.integral_gain > 0.0,
        "{gains:?}"
    );
    assert_eq!(gains.derivative_gain, 0.0);
}
