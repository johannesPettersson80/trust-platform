// The rules of N3.3 and N2.3 with the numbers of their specification clauses. Sample interval Ts = 0.1 s throughout.

use super::*;

/// A candidate of the given form with its errors, its reached fraction and its fitted delay in seconds. The stability of
/// the form comes from its shape, as the fits give it.
fn candidate(
    kind: LowOrderModelKind,
    normalized_error: f64,
    final_normalized_error: f64,
    reached_fraction: Option<f64>,
    dead_time_seconds: Option<f64>,
) -> CheckedCandidate {
    let time_constant_seconds = match kind {
        LowOrderModelKind::StaticGain | LowOrderModelKind::Integrating => None,
        _ => Some(4.0),
    };
    let second_time_constant_seconds = match kind {
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => Some(4.0),
        _ => None,
    };
    CheckedCandidate {
        model: IdentifiedModel {
            kind,
            gain: 1.0,
            time_constant_seconds,
            second_time_constant_seconds,
            dead_time_seconds,
            normalized_error,
            normalized_estimation_error: None,
            normalized_validation_error: None,
            estimation_residuals: None,
            validation_residuals: None,
            uncertainty: None,
            stable: is_stable_form(kind, time_constant_seconds, second_time_constant_seconds),
            on_search_boundary: false,
        },
        final_normalized_error,
        reached_fraction,
        sample_interval_seconds: 0.1,
    }
}

#[test]
fn the_simplest_acceptable_form_is_selected_within_the_policy_limit() {
    // N3.2, N3.3 and N2.3: within the limit 0.15 on both checks, and at least 63 % of the final change where defined.
    let acceptable = [
        candidate(LowOrderModelKind::Integrating, 0.02, 0.02, None, None),
        candidate(LowOrderModelKind::FirstOrder, 0.10, 0.12, Some(0.8), None),
        candidate(
            LowOrderModelKind::FirstOrderDeadTime,
            0.04,
            0.04,
            Some(0.9),
            None,
        ),
    ];
    // The first order has no delay, so the resolved-delay rule does not apply: it is the simplest acceptable form.
    let selected = select(&acceptable, 0.15).expect("a form is acceptable");
    assert_eq!(selected.kind, LowOrderModelKind::FirstOrder);

    // A static gain is the simplest form of all, when it is acceptable.
    let static_gain = [
        candidate(LowOrderModelKind::FirstOrder, 0.05, 0.05, Some(0.9), None),
        candidate(LowOrderModelKind::StaticGain, 0.01, 0.01, Some(1.0), None),
    ];
    assert_eq!(
        select(&static_gain, 0.15)
            .expect("a form is acceptable")
            .kind,
        LowOrderModelKind::StaticGain
    );

    // A held step that reached only 50 % of its final change is not acceptable: the delayed form is selected.
    let partial = [
        candidate(LowOrderModelKind::FirstOrder, 0.04, 0.04, Some(0.5), None),
        candidate(
            LowOrderModelKind::FirstOrderDeadTime,
            0.05,
            0.05,
            Some(0.7),
            None,
        ),
    ];
    assert_eq!(
        select(&partial, 0.15).expect("a form is acceptable").kind,
        LowOrderModelKind::FirstOrderDeadTime
    );
}

#[test]
fn a_form_whose_final_set_misses_the_limit_is_not_within_it() {
    // N3.2: the final set has its own limit, whatever the whole-response error of the form.
    let late_miss = [candidate(
        LowOrderModelKind::FirstOrder,
        0.05,
        0.2,
        Some(0.9),
        None,
    )];
    assert!(select(&late_miss, 0.15).is_none());
}

#[test]
fn a_form_within_the_limit_but_under_63_percent_is_the_partial_choice_for_its_refusal() {
    // N2.3 and A10.3: a form within the limit that reached less than 63 % is not acceptable. With no acceptable form, the
    // simplest form within the limit is returned, and the caller refuses it naming its fraction.
    let short_hold = [candidate(
        LowOrderModelKind::FirstOrder,
        0.04,
        0.04,
        Some(0.5),
        None,
    )];
    let selected = select(&short_hold, 0.15).expect("a form is within the limit");
    assert_eq!(selected.kind, LowOrderModelKind::FirstOrder);
}

#[test]
fn the_integrating_form_needs_no_fraction_to_be_acceptable() {
    // N2.3: a fraction is not defined for an integrator, so its acceptance is the error limit alone.
    let integrating = [candidate(
        LowOrderModelKind::Integrating,
        0.03,
        0.03,
        None,
        Some(0.5),
    )];
    let selected = select(&integrating, 0.15).expect("the integrator is within the limit");
    assert_eq!(selected.kind, LowOrderModelKind::Integrating);
    assert!(selected.stable, "an integrator is a stable form (N2.1)");
}

#[test]
fn a_resolved_delay_wins_when_it_is_long_and_improves_the_error_below_98_percent() {
    // N3.3: L = 5·Ts = 0.5 s is longer than 2·Ts, and the delayed error 0.09 is 0.9 × the first order's 0.10, which is
    // below 98 %. The first order with delay is selected.
    let candidates = [
        candidate(LowOrderModelKind::FirstOrder, 0.10, 0.10, Some(0.9), None),
        candidate(
            LowOrderModelKind::FirstOrderDeadTime,
            0.09,
            0.09,
            Some(0.9),
            Some(0.5),
        ),
    ];
    let selected = select(&candidates, 0.15).expect("a form is acceptable");
    assert_eq!(selected.kind, LowOrderModelKind::FirstOrderDeadTime);
    assert_eq!(selected.dead_time_seconds, Some(0.5));
}

#[test]
fn an_unresolved_delay_loses_to_the_first_order() {
    // N3.3: L = 1.5·Ts = 0.15 s is not longer than 2·Ts, so the first order is selected whatever its error.
    let candidates = [
        candidate(LowOrderModelKind::FirstOrder, 0.10, 0.10, Some(0.9), None),
        candidate(
            LowOrderModelKind::FirstOrderDeadTime,
            0.09,
            0.09,
            Some(0.9),
            Some(0.15),
        ),
    ];
    let selected = select(&candidates, 0.15).expect("a form is acceptable");
    assert_eq!(selected.kind, LowOrderModelKind::FirstOrder);
}

#[test]
fn a_delay_that_does_not_improve_the_error_enough_loses_to_the_first_order() {
    // N3.3: the delayed error 0.099 is 0.99 × the first order's 0.10, which is not below 98 %. The first order is
    // selected even though the delay is resolved.
    let candidates = [
        candidate(LowOrderModelKind::FirstOrder, 0.10, 0.10, Some(0.9), None),
        candidate(
            LowOrderModelKind::FirstOrderDeadTime,
            0.099,
            0.099,
            Some(0.9),
            Some(0.5),
        ),
    ];
    let selected = select(&candidates, 0.15).expect("a form is acceptable");
    assert_eq!(selected.kind, LowOrderModelKind::FirstOrder);
}

#[test]
fn the_resolved_delay_needs_both_forms_to_be_acceptable() {
    // N3.3: the exception compares the first order with delay with the first order. A first order that is not acceptable
    // (its fraction is 0.5) leaves the delayed form as the simplest acceptable one.
    let candidates = [
        candidate(LowOrderModelKind::FirstOrder, 0.10, 0.10, Some(0.5), None),
        candidate(
            LowOrderModelKind::FirstOrderDeadTime,
            0.09,
            0.09,
            Some(0.9),
            Some(0.5),
        ),
    ];
    let selected = select(&candidates, 0.15).expect("a form is acceptable");
    assert_eq!(selected.kind, LowOrderModelKind::FirstOrderDeadTime);
}

#[test]
fn an_unstable_form_is_not_a_candidate_and_a_stable_shape_needs_positive_time_constants() {
    // N2.1 item 5: a second-order form needs a1 > 0 and a2 > 0, which positive time constants give. A form with a
    // negative time constant is unstable and is not chosen.
    assert!(!is_stable_form(
        LowOrderModelKind::FirstOrder,
        Some(-1.0),
        None
    ));
    assert!(!is_stable_form(
        LowOrderModelKind::SecondOrder,
        Some(2.0),
        Some(-1.0)
    ));
    assert!(is_stable_form(
        LowOrderModelKind::SecondOrderDeadTime,
        Some(2.0),
        Some(3.0)
    ));
    assert!(is_stable_form(LowOrderModelKind::Integrating, None, None));
    assert!(is_stable_form(LowOrderModelKind::StaticGain, None, None));

    let mut unstable = candidate(LowOrderModelKind::FirstOrder, 0.02, 0.02, Some(0.9), None);
    unstable.model.stable = false;
    assert!(select(&[unstable], 0.15).is_none());
}
