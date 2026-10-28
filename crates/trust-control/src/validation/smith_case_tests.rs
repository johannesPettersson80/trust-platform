//! Tests of the Smith predictor's part of Validate (specification 35, C9): the model values a candidate writes into a run, the predictor's
//! observed channels when the program uses the library block, and the verdict on its flags and correction at each sample, with the
//! sentence that says why a run fails (C9; Specification 37 §2.2 and §2.5).

#![allow(clippy::float_cmp)] // The verdict's times and values are the samples' own numbers, compared exactly.

use std::collections::BTreeMap;

use super::*;
use crate::commissioning::fixture_loops::smith_predictor_loop;
use crate::identification::connected_record::ValidationValue;
use crate::loop_definition::{LoopDefinition, Parameter, ProjectSeedBlock, RealType};
use crate::recording::channels::ChannelQuality;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_candidate::{served_smith, smith_candidate};
use crate::tuning::project_seed::{SmithLibraryBlock, SmithTerms};
use crate::tuning::structure::ParallelGains;

/// The library block of the tests: the predictor's result, its maximum correction, and its operating point, by their variables.
fn library_block() -> SmithLibraryBlock {
    SmithLibraryBlock {
        output_symbol: "Main.Out".to_owned(),
        maximum_correction_symbol: "Main.MaxC".to_owned(),
        operating_input_symbol: "Main.U0".to_owned(),
        operating_output_symbol: "Main.Y0".to_owned(),
    }
}

/// The fixture loop with the library block named, and the block's operating point bound as parameters (C9, D-18).
fn with_library_block() -> LoopDefinition {
    let mut loop_definition = smith_predictor_loop();
    let project = loop_definition
        .controller
        .project
        .as_mut()
        .expect("the fixture declares a program's law");
    if let Some(ProjectSeedBlock::Smith(seed)) = project.seed.as_mut() {
        seed.library_block = Some(library_block());
    }
    let binding = loop_definition
        .parameter_binding
        .as_mut()
        .expect("the fixture binds its parameters");
    for (symbol, unit, minimum, maximum) in
        [("Main.U0", "%", 0.0, 100.0), ("Main.Y0", "%", 0.0, 80.0)]
    {
        binding.parameters.push(Parameter {
            symbol: symbol.to_owned(),
            iec_type: RealType::Lreal,
            unit: unit.to_owned(),
            convention: "Smith predictor operating point".to_owned(),
            minimum,
            maximum,
        });
    }
    loop_definition
}

/// The PI of the tests' candidate.
fn pi() -> ParallelGains {
    ParallelGains {
        proportional_gain: 2.0,
        integral_gain: 0.2,
        derivative_gain: 0.0,
    }
}

/// One sample of a run at `time` seconds, with the given channel values (`None` is a missing value).
fn sample(time: f64, values: &[(&str, Option<f64>)]) -> ValidationSample {
    ValidationSample {
        time_seconds: time,
        sequence: 0,
        values: values
            .iter()
            .map(|(symbol, value)| {
                (
                    (*symbol).to_owned(),
                    ValidationValue {
                        value: *value,
                        quality: ChannelQuality::Good,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    }
}

/// The predictor's channels of a sample at `time`: the flags, the correction and the program's maximum.
fn predictor_sample(
    time: f64,
    history: f64,
    active: f64,
    correction: f64,
    maximum: f64,
) -> ValidationSample {
    sample(
        time,
        &[
            ("Main.Out.HistoryComplete", Some(history)),
            ("Main.Out.Active", Some(active)),
            ("Main.Out.Correction", Some(correction)),
            ("Main.MaxC", Some(maximum)),
        ],
    )
}

#[test]
fn the_observations_name_the_flags_the_correction_and_the_maximum_only_with_the_library_block() {
    assert_eq!(
        observations(&smith_predictor_loop()),
        [] as [crate::validation::isolated_run_protocol::IsolatedRunObservation; 0]
    );
    let observed: Vec<(String, String)> = observations(&with_library_block())
        .into_iter()
        .map(|observation| (observation.symbol, observation.unit))
        .collect();
    assert_eq!(
        observed,
        vec![
            ("Main.Out.Active".to_owned(), "flag".to_owned()),
            ("Main.Out.HistoryComplete".to_owned(), "flag".to_owned()),
            ("Main.Out.Correction".to_owned(), "%".to_owned()),
            ("Main.MaxC".to_owned(), "%".to_owned()),
        ]
    );
}

#[test]
fn a_candidate_writes_the_model_values_and_the_operating_point_of_the_library_block() {
    let loop_definition = with_library_block();
    let seed = served_smith(&loop_definition)
        .expect("the loop declares a Smith predictor seed")
        .clone();
    let terms = SmithTerms {
        gain: 0.2,
        time_constant_seconds: 5.0,
        delay_seconds: 1.0,
        operating: (75.0, 35.0),
    };
    let candidate = LawCandidate::Project(smith_candidate(
        &seed,
        CandidateSource::ConstrainedComparison,
        pi(),
        terms,
    ));
    let written: Vec<(String, f64)> = overrides(&loop_definition, &candidate)
        .expect("the model values are written")
        .into_iter()
        .map(|item| (item.symbol, item.value))
        .collect();
    assert_eq!(
        written,
        vec![
            ("Main.Program.ModelGain".to_owned(), 0.2),
            ("Main.Program.ModelTau".to_owned(), 5.0),
            ("Main.Program.ModelDelay".to_owned(), 1.0),
            ("Main.U0".to_owned(), 75.0),
            ("Main.Y0".to_owned(), 35.0),
        ]
    );
}

#[test]
fn a_predictor_of_the_program_writes_three_model_values_and_no_operating_point() {
    let loop_definition = smith_predictor_loop();
    let seed = served_smith(&loop_definition)
        .expect("the loop declares a Smith predictor seed")
        .clone();
    let terms = SmithTerms {
        gain: 0.2,
        time_constant_seconds: 5.0,
        delay_seconds: 1.0,
        operating: (0.0, 0.0),
    };
    let candidate = LawCandidate::Project(smith_candidate(
        &seed,
        CandidateSource::ConstrainedComparison,
        pi(),
        terms,
    ));
    let written = overrides(&loop_definition, &candidate).expect("the model values are written");
    assert_eq!(written.len(), 3);
    assert_eq!(
        observations(&loop_definition),
        [] as [crate::validation::isolated_run_protocol::IsolatedRunObservation; 0]
    );
}

#[test]
fn a_run_whose_predictor_is_active_below_its_maximum_passes() {
    let samples = [
        predictor_sample(0.0, 1.0, 1.0, 0.5, 5.0),
        predictor_sample(0.1, 1.0, 1.0, -2.0, 5.0),
    ];
    let (status, time, reason) = judge(&samples, &library_block(), "%");
    assert_eq!(status, RequirementStatus::Pass);
    assert_eq!(time, None);
    assert_eq!(
        reason,
        "The predictor is active at every sample and its correction stays below its maximum."
    );
}

#[test]
fn a_history_that_is_incomplete_fails_the_run_with_the_history_reason() {
    let samples = [
        predictor_sample(0.0, 1.0, 1.0, 0.0, 5.0),
        predictor_sample(0.5, 0.0, 0.0, 0.0, 5.0),
    ];
    let (status, time, reason) = judge(&samples, &library_block(), "%");
    assert_eq!(status, RequirementStatus::Fail);
    assert_eq!(time, Some(0.5));
    assert_eq!(
        reason,
        "The predictor's history is incomplete at 0.500 s, so the predictor is not active."
    );
}

#[test]
fn a_predictor_that_is_not_enabled_with_a_complete_history_fails_the_run_with_the_enabled_reason() {
    let samples = [predictor_sample(1.0, 1.0, 0.0, 0.0, 5.0)];
    let (status, time, reason) = judge(&samples, &library_block(), "%");
    assert_eq!(status, RequirementStatus::Fail);
    assert_eq!(time, Some(1.0));
    assert_eq!(
        reason,
        "The predictor is not enabled at 1.000 s, although its history is complete."
    );
}

#[test]
fn a_correction_that_reaches_a_positive_maximum_fails_the_run() {
    let samples = [predictor_sample(0.2, 1.0, 1.0, -5.0, 5.0)];
    let (status, time, reason) = judge(&samples, &library_block(), "%");
    assert_eq!(status, RequirementStatus::Fail);
    assert_eq!(time, Some(0.2));
    assert_eq!(
        reason,
        "The predictor's correction reaches its maximum of 5 % at 0.200 s."
    );
}

#[test]
fn a_maximum_that_is_not_positive_bounds_nothing() {
    let samples = [predictor_sample(0.0, 1.0, 1.0, 50.0, 0.0)];
    let (status, _, _) = judge(&samples, &library_block(), "%");
    assert_eq!(status, RequirementStatus::Pass);
}

#[test]
fn a_missing_flag_leaves_the_run_inconclusive_at_its_first_sample() {
    let samples = [sample(
        0.3,
        &[
            ("Main.Out.HistoryComplete", None),
            ("Main.Out.Active", Some(1.0)),
            ("Main.Out.Correction", Some(0.0)),
            ("Main.MaxC", Some(5.0)),
        ],
    )];
    let (status, time, reason) = judge(&samples, &library_block(), "%");
    assert_eq!(status, RequirementStatus::Inconclusive);
    assert_eq!(time, Some(0.3));
    assert_eq!(
        reason,
        "The predictor's flags or correction are missing at 0.300 s."
    );
}
