//! The pole-zero map (`poleZero`, B9.9). Ported assertions name the TypeScript test they come from.

use serde_json::{json, Value};

use super::{
    pole_zero_answer, CONTINUOUS_DOMAIN, DISCRETE_DOMAIN, HIDDEN_MODE, NO_COMMON_FACTOR, REMOVED,
};
use crate::messages::{AnalysisInputs, PoleDomain, PoleZeroView};
use crate::model::{decode_general_model, GeneralModel};

fn channel(id: &str) -> Value {
    json!({ "id": id, "unit": "u", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } })
}

/// A continuous single-input, single-output transfer function `numerator / denominator`.
fn transfer(numerator: &[f64], denominator: &[f64]) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "tf", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0, "inputs": [channel("u")], "outputs": [channel("y")],
        "numerator": numerator, "denominator": denominator,
    }))
    .expect("the transfer function is valid")
}

/// The SISO state-space model of `TypeScript`'s `converts SISO continuous state-space without cancelling hidden modes`:
/// `a = diag(−1, 1)`, `b = [1, 0]`, `c = [1, 0]`, whose transfer function is `(s − 1)/(s² − 1)`.
fn hidden_mode_state_space() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "hidden", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0, "inputs": [channel("u")], "outputs": [channel("y")],
        "states": [channel("x1"), channel("x2")],
        "a": [[-1.0, 0.0], [0.0, 1.0]], "b": [[1.0], [0.0]], "c": [[1.0, 0.0]], "d": [[0.0]],
    }))
    .expect("the state-space model is valid")
}

/// A continuous two-input, two-output state-space model with `a = diag(−1, −2)`.
fn rectangular() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "rect", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0, "inputs": [channel("u1"), channel("u2")],
        "outputs": [channel("y1"), channel("y2")], "states": [channel("x1"), channel("x2")],
        "a": [[-1.0, 0.0], [0.0, -2.0]], "b": [[1.0, 0.0], [0.0, 1.0]],
        "c": [[1.0, 0.0], [0.0, 1.0]], "d": [[0.0, 0.0], [0.0, 0.0]],
    }))
    .expect("the rectangular model is valid")
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-9
}

fn view(view: PoleZeroView) -> AnalysisInputs {
    AnalysisInputs {
        view: Some(view),
        ..AnalysisInputs::default()
    }
}

#[test]
fn a_minimal_view_removes_the_exact_common_factor_and_the_hidden_mode_stays_a_warning() {
    // Ported from `keeps the full unstable model when the engineer selects a minimal pole-zero view` (control-studio-general-model-session.test.ts):
    // (s − 1)/(s² − 1) has the common factor s − 1; the unreduced denominator is [1, 0, −1], the minimal one [1, 1], the cancelled
    // factor [1, −1], and the hidden mode is unstable in both views.
    let model = transfer(&[1.0, -1.0], &[1.0, 0.0, -1.0]);
    let unreduced =
        pole_zero_answer(&model, &AnalysisInputs::default()).expect("the unreduced view");
    assert_eq!(unreduced.view, PoleZeroView::Unreduced);
    assert_eq!(unreduced.domain, PoleDomain::S);
    let unreduced_polynomials = unreduced
        .unreduced
        .expect("a SISO model has its polynomials");
    assert_eq!(unreduced_polynomials.denominator, vec![1.0, 0.0, -1.0]);
    let minimal = unreduced.minimal.expect("the minimal polynomials");
    assert_eq!(minimal.denominator, vec![1.0, 1.0]);
    assert_eq!(minimal.numerator, vec![1.0]);
    assert_eq!(unreduced.cancelled_factor, Some(vec![1.0, -1.0]));
    assert!(unreduced.hidden_unstable_mode);
    assert_eq!(
        unreduced.poles.len(),
        2,
        "the unreduced view keeps both poles"
    );
    assert_eq!(unreduced.zeros.len(), 1);
    assert_eq!(
        unreduced.summary,
        vec![
            HIDDEN_MODE.to_owned(),
            REMOVED.to_owned(),
            CONTINUOUS_DOMAIN.to_owned()
        ]
    );

    let reduced = pole_zero_answer(&model, &view(PoleZeroView::Minimal)).expect("the minimal view");
    assert_eq!(reduced.view, PoleZeroView::Minimal);
    assert_eq!(
        reduced.poles.len(),
        1,
        "the minimal view keeps the pole at −1"
    );
    assert!(close(reduced.poles[0].real, -1.0));
    assert_eq!(reduced.zeros.len(), 0);
    assert!(
        reduced.hidden_unstable_mode,
        "the warning does not depend on the view"
    );
    // Ported: the selected model is not replaced: the answer's unreduced polynomials are the model's own.
    assert_eq!(
        reduced.unreduced.expect("the polynomials").denominator,
        vec![1.0, 0.0, -1.0]
    );
}

#[test]
fn a_state_space_model_converts_without_cancelling_its_hidden_modes() {
    // Ported from `converts SISO continuous state-space without cancelling hidden modes` (control-studio-general-model.test.ts):
    // the unreduced denominator is [1, 0, −1] and the numerator [1, −1]; the minimal denominator is [1, 1].
    let answer = pole_zero_answer(&hidden_mode_state_space(), &AnalysisInputs::default())
        .expect("the conversion qualifies");
    let unreduced = answer.unreduced.expect("the polynomials");
    assert_eq!(unreduced.denominator.len(), 3);
    assert!(close(unreduced.denominator[0], 1.0));
    assert!(close(unreduced.denominator[1], 0.0));
    assert!(close(unreduced.denominator[2], -1.0));
    assert_eq!(unreduced.numerator.len(), 2);
    assert!(close(unreduced.numerator[0], 1.0) && close(unreduced.numerator[1], -1.0));
    let minimal = answer.minimal.expect("the minimal polynomials");
    assert!(close(minimal.denominator[0], 1.0) && close(minimal.denominator[1], 1.0));
    assert!(answer.hidden_unstable_mode);
}

#[test]
fn a_multi_channel_model_has_its_eigenvalues_as_poles_no_zeros_and_only_the_unreduced_view() {
    let answer = pole_zero_answer(&rectangular(), &AnalysisInputs::default())
        .expect("the multi-channel view");
    let mut real: Vec<f64> = answer.poles.iter().map(|pole| pole.real).collect();
    real.sort_by(f64::total_cmp);
    assert_eq!(real.len(), 2);
    assert!(close(real[0], -2.0) && close(real[1], -1.0));
    assert_eq!(answer.zeros.len(), 0);
    assert!(
        answer.unreduced.is_none(),
        "no single transfer function is shown"
    );
    assert_eq!(answer.summary, vec![CONTINUOUS_DOMAIN.to_owned()]);
}

#[test]
fn the_minimal_view_of_a_multi_channel_model_names_the_view_as_its_input_failure() {
    let refused = pole_zero_answer(&rectangular(), &view(PoleZeroView::Minimal))
        .expect_err("a minimal view needs one input and one output");
    assert!(refused.message.contains("view"), "{}", refused.message);
    assert_eq!(refused.next_action, crate::failure::NextAction::None);
}

#[test]
fn a_discrete_model_is_shown_on_the_z_plane_with_its_unit_circle() {
    let model = decode_general_model(&json!({
        "kind": "discrete-state-space", "revision": "discrete", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0, "inputs": [channel("u")], "outputs": [channel("y")],
        "states": [channel("x")], "a": [[0.5]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]], "sampleTimeSeconds": 0.1,
    }))
    .expect("the discrete model is valid");
    let answer = pole_zero_answer(&model, &AnalysisInputs::default()).expect("the z-plane view");
    assert_eq!(answer.domain, PoleDomain::Z);
    assert_eq!(answer.poles.len(), 1);
    assert!(close(answer.poles[0].real, 0.5));
    assert!(
        !answer.hidden_unstable_mode,
        "|0.5| is inside the unit circle"
    );
    assert_eq!(
        answer.summary.last().map(String::as_str),
        Some(DISCRETE_DOMAIN)
    );
}

#[test]
fn a_model_without_an_exact_common_factor_says_so_and_both_views_are_the_same() {
    let model = transfer(&[1.0], &[1.0, 2.0]);
    let answer = pole_zero_answer(&model, &view(PoleZeroView::Minimal)).expect("the minimal view");
    assert_eq!(answer.cancelled_factor, Some(vec![1.0]));
    assert_eq!(answer.summary[0], NO_COMMON_FACTOR);
    assert!(!answer.hidden_unstable_mode);
    assert_eq!(
        answer.minimal.expect("the minimal polynomials").denominator,
        vec![1.0, 2.0]
    );
}

#[test]
fn the_four_discretizations_of_a_stable_plant_show_one_pole_on_the_z_plane_inside_the_unit_circle()
{
    // control-studio-numerics: "qualifies four discretizations and z-plane root-locus stability" (specification 35 B9.9, numerical annex
    // N4.6). dx/dt = −2x + 3u sampled at 0.1 s by each method has the single pole a of the method's own formula (the discretization
    // itself is pinned in trust-control-math), which the z-plane view shows inside the unit circle with no hidden mode.
    use trust_control_math::discretize::{discretize_state_space, DiscretizationMethod};
    use trust_control_math::model::StateSpace;
    let continuous = StateSpace::new(
        vec![vec![-2.0]],
        vec![vec![3.0]],
        vec![vec![1.0]],
        vec![vec![0.0]],
    )
    .expect("consistent shapes");
    let exact = (-0.2_f64).exp();
    for (method, pole) in [
        (DiscretizationMethod::ForwardEuler, 0.8),
        (DiscretizationMethod::BackwardEuler, 1.0 / 1.2),
        (DiscretizationMethod::Tustin, 0.9 / 1.1),
        (DiscretizationMethod::ExactZoh, exact),
    ] {
        let discrete =
            discretize_state_space(&continuous, 0.1, method).expect("the plant qualifies");
        let model = decode_general_model(&json!({
            "kind": "discrete-state-space", "revision": "discretized", "source": "provided", "validation": "unvalidated",
            "evidence": [], "delaySeconds": 0.0, "inputs": [channel("u")], "outputs": [channel("y")],
            "states": [channel("x")], "a": discrete.system.a, "b": discrete.system.b,
            "c": discrete.system.c, "d": discrete.system.d,
            "sampleTimeSeconds": discrete.sample_time_seconds,
        }))
        .expect("the discrete model is valid");
        let answer =
            pole_zero_answer(&model, &AnalysisInputs::default()).expect("the z-plane view");
        assert_eq!(answer.domain, PoleDomain::Z, "{method:?}");
        assert_eq!(answer.poles.len(), 1, "{method:?}");
        assert!(
            close(answer.poles[0].real, pole),
            "{method:?}: {:?}",
            answer.poles
        );
        assert!(close(answer.poles[0].imaginary, 0.0), "{method:?}");
        assert!(answer.poles[0].real.abs() < 1.0, "{method:?}");
        assert!(!answer.hidden_unstable_mode, "{method:?}");
    }
}
