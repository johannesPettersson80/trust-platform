#![allow(clippy::float_cmp)] // The rounded results are whole numbers, compared exactly on purpose.

use super::*;

fn score(whole_within_limit: bool, status: ScoreStatus) -> IndependentResponseScore {
    IndependentResponseScore {
        normalized_error: 0.0,
        final_normalized_error: 0.0,
        whole_within_limit,
        final_within_limit: status == ScoreStatus::Pass,
        status,
    }
}

#[test]
fn the_independent_check_says_what_passed_or_what_to_change() {
    assert_eq!(
        independent_response_reason(&score(true, ScoreStatus::Pass)),
        "Frozen predictions agree with independent plant measurements over the whole response and its final 10 %."
    );
    let whole = independent_response_reason(&score(false, ScoreStatus::Fail));
    assert!(whole.contains("over the whole response"));
    assert!(whole.contains("a different method, a larger step, or a longer duration"));
    let final_tenth = independent_response_reason(&score(true, ScoreStatus::Fail));
    assert!(final_tenth.contains("not its final 10 %"));
}

#[test]
fn a_gap_is_named_with_its_length_and_its_start() {
    assert_eq!(
        gap_phrase(SampleGap {
            start_seconds: 12.3,
            length_seconds: 0.42,
        }),
        "a gap of 0.42 s at 12.3 s"
    );
}

#[test]
fn a_gain_of_the_wrong_sign_names_the_direction_in_words_and_the_loop_setup() {
    let text = gain_sign_contradiction(-2.0, "", ControllerAction::Direct)
        .expect("a negative gain contradicts a direct action");
    assert_eq!(
        text,
        "The identified gain is −2 (the measurement falls when the output rises), but the loop is set up to increase the output while the measurement is below the setpoint. Check the measured output, the plant input and the controller action in the loop setup."
    );
    assert!(gain_sign_contradiction(2.0, "%", ControllerAction::Direct).is_none());
    assert!(gain_sign_contradiction(0.0, "%", ControllerAction::Reverse).is_none());
}

#[test]
fn a_response_below_twenty_times_the_noise_is_refused_with_both_values() {
    assert_eq!(
        noise_refusal(0.5, 0.1, "degC", ExcitationWord::Step).as_deref(),
        Some("The response (0.5 degC) is smaller than 20 times the measured noise (0.1 degC RMS). Permit a larger step in Limits…, or choose another method.")
    );
    assert!(
        noise_refusal(5.0, 0.1, "degC", ExcitationWord::Step).is_none(),
        "large enough"
    );
    assert!(
        noise_refusal(0.5, 0.0, "degC", ExcitationWord::Step).is_none(),
        "no noise: no check"
    );
}

#[test]
fn a_partial_response_names_its_fraction_and_the_hold_its_time_constant_suggests() {
    // 50 % reached after 10 min; the hold L + 3τ is 60 s + 900 s, which is 16 min.
    assert_eq!(
        partial_response_refusal(0.5, 600.0, 60.0, 300.0),
        "The response reached 50 % of its fitted final change in 10 min; hold the step for about 16 min (Limits… → maximum duration)."
    );
}

#[test]
fn minutes_are_printed_to_two_significant_digits_with_an_exact_tie_going_up() {
    assert_eq!(minutes_text(750.0), "13", "12.5 min is an exact tie");
    assert_eq!(minutes_text(720.0), "12");
}

#[test]
fn math_round_sends_an_exact_half_toward_positive_infinity() {
    assert_eq!(js_round(2.5), 3.0);
    assert_eq!(js_round(-2.5), -2.0);
    assert_eq!(js_round(62.4), 62.0);
}

#[test]
fn the_relay_sentences_are_the_ones_the_relay_tests_match() {
    assert!(relay_record_text(RelayRecordError::IntermediateOutput)
        .contains("clipped or intermediate output values"));
    assert!(relay_record_text(RelayRecordError::TooFewTransitions).contains("transitions"));
    assert_eq!(
        relay_rejection_text(RelayRejection::Spread(RelayQuantity::Period)),
        "Relay period spread exceeds 10% of its mean. Record more settled cycles or use another permitted identification method."
    );
}

#[test]
fn a_project_scenario_refuses_an_explicit_excitation_as_a_duplicate() {
    // C12: the project owns the scenario, so Control Studio writes no duplicate excitation.
    assert_eq!(
        policy_error_text(&PolicyError::DuplicateExcitation),
        "The project owns this experiment's scenario, so Control Studio writes no duplicate excitation. Choose Automatic to observe the project's scenario."
    );
}

#[test]
fn a_record_without_an_input_change_names_the_missing_nonzero_step() {
    // N2.6: a record with no input change reports the missing nonzero step.
    assert_eq!(
        MISSING_NONZERO_STEP,
        "The recording is missing a nonzero step: the plant input never changes, so there is no input change to fit. Check that the experiment moves the plant input, then identify again."
    );
}

#[test]
fn an_even_sampling_refusal_names_the_gap_it_found() {
    assert_eq!(
        even_sampling_text(EvenSamplingError::Gap(SampleGap {
            start_seconds: 12.3,
            length_seconds: 0.42,
        })),
        "The independent recording has missing or irregular scans: a gap of 0.42 s at 12.3 s."
    );
    assert_eq!(
        even_sampling_text(EvenSamplingError::TooFew),
        "Model validation requires at least 30 complete scans."
    );
}
