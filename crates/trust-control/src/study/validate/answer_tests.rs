//! Tests of the parts of the Validate answer that the page draws (specification 35, B5.4 and B9.6): the plot of the target-response
//! case, the measured range of an operating-range requirement, the Automatic mark, the model-agreement row and the table's words.

use std::collections::BTreeMap;

use super::*;
use crate::identification::connected_record::{
    RunConditions, RunEvidence, ValidationChannel, ValidationValue,
};
use crate::identification::independent_validation::RequirementStatus;
use crate::study::validate::tests::fixture_loop;
use crate::validation::run_validation::{ErrorReference, ErrorStatistic, Movement, Window};

/// A test index as a float, for the times and values of the long run.
fn at(index: usize) -> f64 {
    f64::from(u32::try_from(index).expect("a test index fits in u32"))
}

/// One sample at `time_seconds` whose channel `symbol` holds `value` with `quality`.
fn sample(
    time_seconds: f64,
    symbol: &str,
    value: Option<f64>,
    quality: ChannelQuality,
) -> ValidationSample {
    ValidationSample {
        time_seconds,
        sequence: 0,
        values: BTreeMap::from([(symbol.to_owned(), ValidationValue { value, quality })]),
    }
}

/// A run of the given samples, with the duration and the sample times that its samples carry.
fn run_of(id: &str, samples: Vec<ValidationSample>, duration_seconds: f64) -> ValidationRun {
    ValidationRun {
        id: id.to_owned(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds,
            sample_times_seconds: samples.iter().map(|sample| sample.time_seconds).collect(),
            seed: 1,
            model_revision: "model-4".to_owned(),
            program_revision: "st-1".to_owned(),
            parameter_schema_revision: "params-1".to_owned(),
        },
        channels: vec![ValidationChannel {
            id: "Main.PV".to_owned(),
            unit: "m".to_owned(),
        }],
        samples,
    }
}

/// B9.6, B4.5: the plot keeps every point of a short run, and a sample that is not good is a gap, never a value.
#[test]
fn the_plot_keeps_every_point_of_a_short_run_and_leaves_a_sample_that_is_not_good_a_gap() {
    let loop_definition = fixture_loop();
    let symbol = loop_definition.signals.process_variable.symbol.clone();
    let candidate = run_of(
        "candidate",
        vec![
            sample(0.0, &symbol, Some(0.0), ChannelQuality::Good),
            sample(1.0, &symbol, Some(9.0), ChannelQuality::Bad),
            sample(2.0, &symbol, Some(1.5), ChannelQuality::Good),
            sample(3.0, &symbol, Some(2.0), ChannelQuality::Forced),
        ],
        3.0,
    );
    let current = run_of(
        "current",
        vec![
            sample(0.0, &symbol, Some(0.0), ChannelQuality::Good),
            sample(1.0, &symbol, Some(0.5), ChannelQuality::Good),
        ],
        1.0,
    );
    let plot = plot(&loop_definition, Some(&current), Some(&candidate))
        .expect("the candidate has samples");
    assert_eq!(plot.title, "Measured response");
    assert_eq!(plot.role, "Measurement");
    assert_eq!(plot.unit, "m");
    assert_eq!(plot.tuned.name, "Tuned");
    assert_eq!(plot.tuned.time_seconds, vec![0.0, 1.0, 2.0, 3.0]);
    assert_eq!(plot.tuned.values, vec![Some(0.0), None, Some(1.5), None]);
    let current_series = plot.current.expect("the current settings' run has samples");
    assert_eq!(current_series.name, "Current");
    assert_eq!(current_series.time_seconds, vec![0.0, 1.0]);
    assert_eq!(current_series.values, vec![Some(0.0), Some(0.5)]);
}

/// B4.5: a run of more than 6,000 points keeps each group's lowest and highest value, so a peak is not lost, and its first and
/// last point.
#[test]
fn a_long_run_keeps_each_groups_extremes_and_its_first_and_last_point() {
    let loop_definition = fixture_loop();
    let symbol = loop_definition.signals.process_variable.symbol.clone();
    let count = 6_001;
    let samples: Vec<ValidationSample> = (0..count)
        .map(|index| {
            // One spike, which the reduction must keep.
            let value = if index == 3_001 { 100.0 } else { at(index) };
            sample(
                at(index) / 100.0,
                &symbol,
                Some(value),
                ChannelQuality::Good,
            )
        })
        .collect();
    let candidate = run_of("candidate", samples, 60.0);
    let plot = plot(&loop_definition, None, Some(&candidate)).expect("the candidate has samples");
    let times = &plot.tuned.time_seconds;
    assert!(
        times.len() > 1_000 && times.len() <= 2 * 1_000 + 2,
        "each of the 1,000 groups keeps its lowest and its highest point, and the first and the last"
    );
    assert_eq!(times.first(), Some(&0.0));
    assert_eq!(times.last(), Some(&(at(count - 1) / 100.0)));
    assert!(
        plot.tuned.values.contains(&Some(100.0)),
        "the spike is kept"
    );
    assert!(
        times.windows(2).all(|pair| pair[0] < pair[1]),
        "the kept points stay in time order"
    );
    assert!(plot.current.is_none(), "no current run, no current trace");
}

/// B5.4: a plot needs a candidate run with samples; without one there is no plot.
#[test]
fn no_plot_without_a_candidate_run_that_has_samples() {
    let loop_definition = fixture_loop();
    let symbol = loop_definition.signals.process_variable.symbol.clone();
    let empty = run_of("empty", Vec::new(), 1.0);
    let candidate = run_of(
        "candidate",
        vec![sample(0.0, &symbol, Some(1.0), ChannelQuality::Good)],
        1.0,
    );
    assert!(plot(&loop_definition, None, Some(&empty)).is_none());
    assert!(plot(&loop_definition, Some(&candidate), None).is_none());
}

/// B5.4: an operating-range requirement reports the lowest and the highest good value of its channel in its window, and the
/// declared bounds; any other check reports neither.
#[test]
fn an_operating_range_reports_its_measured_range_in_its_window_and_its_declared_bounds() {
    let run = run_of(
        "run",
        vec![
            sample(0.0, "Main.PV", Some(50.0), ChannelQuality::Good),
            sample(1.0, "Main.PV", Some(10.0), ChannelQuality::Good),
            sample(2.0, "Main.PV", Some(99.0), ChannelQuality::Bad),
            sample(3.0, "Main.PV", Some(30.0), ChannelQuality::Good),
            sample(4.0, "Main.PV", Some(-5.0), ChannelQuality::Good),
        ],
        4.0,
    );
    let bounds = RunRequirement {
        id: "process-bounds".to_owned(),
        channel: "Main.PV".to_owned(),
        window: Window {
            start: 1.0,
            end: 3.0,
        },
        check: RunCheck::Bounds {
            minimum: 0.0,
            maximum: 100.0,
        },
    };
    assert_eq!(
        bounds_of(&bounds, &run),
        (
            Some(ValueRange {
                minimum: 10.0,
                maximum: 30.0,
            }),
            Some(ValueRange {
                minimum: 0.0,
                maximum: 100.0,
            }),
        )
    );
    let state = RunRequirement {
        id: "state".to_owned(),
        check: RunCheck::StateBounds {
            minimum: 0.0,
            maximum: 1.0,
        },
        ..bounds.clone()
    };
    assert!(
        bounds_of(&state, &run).0.is_some(),
        "a state range is measured the same way"
    );
    let error = RunRequirement {
        check: RunCheck::Error {
            reference: ErrorReference::Constant(1.0),
            statistic: ErrorStatistic::Maximum,
            maximum: 0.1,
        },
        ..bounds
    };
    assert_eq!(bounds_of(&error, &run), (None, None));
}

/// B5.4, B4.3: a settling time or a maximum error that Automatic generated is marked; a declared limit, and an overshoot, never are.
#[test]
fn a_settling_time_or_maximum_error_that_automatic_generated_is_marked() {
    let criteria = AutomaticCriteria {
        model_revision: "model-4".to_owned(),
        fields: vec!["settlingTimeSeconds".to_owned()],
    };
    let settling = RunCheck::Settling {
        target: 1.0,
        band: 0.1,
        hold_seconds: 5.0,
        deadline_seconds: 10.0,
    };
    let error = RunCheck::Error {
        reference: ErrorReference::Constant(1.0),
        statistic: ErrorStatistic::Maximum,
        maximum: 0.2,
    };
    assert!(generated_limit(&settling, Some(&criteria)));
    assert!(
        !generated_limit(&error, Some(&criteria)),
        "the maximum error was not generated"
    );
    assert!(!generated_limit(&settling, None), "no criteria, no mark");
    let both = AutomaticCriteria {
        fields: vec!["settlingTimeSeconds".to_owned(), "maximumError".to_owned()],
        ..criteria
    };
    assert!(generated_limit(&error, Some(&both)));
    let overshoot = RunCheck::Overshoot {
        initial: 0.0,
        target: 1.0,
        maximum: 0.1,
    };
    assert!(
        !generated_limit(&overshoot, Some(&both)),
        "an overshoot limit is always declared"
    );
}

/// B5.2, N3.4: the model check is the table's first row, its error and its limit in percent of the span.
#[test]
fn the_model_agreement_row_states_the_error_and_the_limit_in_percent_of_the_span() {
    let check = ModelCheck {
        status: RequirementStatus::Pass,
        model_revision: "model-4".to_owned(),
        recording_id: format!("rec-{}", "0".repeat(32)),
        limit: 0.15,
        normalized_error: Some(0.04),
        reason: "The check measured its error against the limit.".to_owned(),
    };
    let row = model_agreement(&check);
    assert_eq!(row.id, "model-check");
    assert_eq!(row.label, "Model agreement");
    assert_eq!(row.status, RequirementStatus::Pass);
    assert!((row.value.expect("the error was measured") - 4.0).abs() < 1e-9);
    assert!((row.limit.expect("the limit is declared") - 15.0).abs() < 1e-9);
    assert_eq!(row.unit, "%");
    assert_eq!(row.limit_side, LimitSide::AtMost);
    assert!(
        row.channel.role.is_empty() && row.channel.unit.is_empty() && row.channel.symbol.is_empty()
    );
    assert!(!row.automatic);
    assert!(!row.guard, "the model check is not a guard");
    assert_eq!(row.measured_range, None);
    assert_eq!(row.limit_range, None);
    assert_eq!(row.reason, check.reason);
    let unmeasured = model_agreement(&ModelCheck {
        normalized_error: None,
        ..check
    });
    assert_eq!(
        unmeasured.value, None,
        "no channel measured an error, so there is no value"
    );
}

/// B9.9: the table's own words are the page specification's (the run names and the Automatic mark).
#[test]
fn the_table_names_its_rows_with_the_words_of_the_page_specification() {
    let words = labels();
    assert_eq!(words.current_settings, "Current settings");
    assert_eq!(words.candidate, "Candidate");
    assert_eq!(words.candidate_overall, "Candidate overall");
    assert_eq!(words.automatic, "Automatic");
    assert_eq!(words.guards, "Ranges, states and transfer");
    assert_eq!(words.guards_limit, "Within declared limits");
}

/// B5.3, B9.9: a guard is a bounds or rate check that the plan generated from the loop's declaration. A requirement the engineer
/// sets, and a settling, error or overshoot check, is never one, even when it is a bounds check.
#[test]
fn a_guard_is_a_generated_bounds_or_rate_check_and_never_an_engineers_requirement() {
    let window = Window {
        start: 0.0,
        end: 10.0,
    };
    let requirement = |id: &str, check: RunCheck| RunRequirement {
        id: id.to_owned(),
        channel: "P1.Y".to_owned(),
        window,
        check,
    };
    let plan = VerificationPlan {
        step_ms: 10,
        duration_ms: 10_000,
        max_total_cycles: 1_000,
        record_every: 1,
        initial_values: BTreeMap::new(),
        observed_symbols: vec!["P1.Y".to_owned()],
        requirements: vec![
            requirement(
                "process-bounds",
                RunCheck::Bounds {
                    minimum: 0.0,
                    maximum: 100.0,
                },
            ),
            requirement(
                "output-rate-up",
                RunCheck::Rate {
                    maximum: 5.0,
                    direction: Some(Movement::Up),
                },
            ),
            requirement(
                "fixed-parameter:P1.Kp",
                RunCheck::Bounds {
                    minimum: 1.0,
                    maximum: 1.0,
                },
            ),
            requirement(
                "response-deadline",
                RunCheck::Settling {
                    target: 50.0,
                    band: 1.0,
                    hold_seconds: 5.0,
                    deadline_seconds: 5.0,
                },
            ),
            requirement(
                "overshoot",
                RunCheck::Overshoot {
                    initial: 0.0,
                    target: 1.0,
                    maximum: 0.1,
                },
            ),
        ],
    };
    for generated in &plan.requirements[..3] {
        assert!(
            is_guard(generated, &plan),
            "{} is generated from the loop's declaration",
            generated.id
        );
    }
    for settled in &plan.requirements[3..] {
        assert!(
            !is_guard(settled, &plan),
            "{} is the engineer's requirement",
            settled.id
        );
    }
    let declared = requirement(
        "load-minimum",
        RunCheck::Bounds {
            minimum: 33.0,
            maximum: 80.0,
        },
    );
    assert!(
        !is_guard(&declared, &plan),
        "a declared bounds check the plan does not generate is not a guard"
    );
}
