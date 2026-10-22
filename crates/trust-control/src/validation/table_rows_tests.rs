//! Tests of the Validate table's rows (specification 35, B5.4 and B9.9): their order, the passing guards counted in one row, their
//! words, their measured and limit texts, their verdicts, and the rows that only some answers have.

use super::*;
use crate::identification::connected_record::RunEvidence;
use crate::messages::RunOutcome;
use crate::validation::texts;

/// The words the engine names the table's rows with (`answer::labels`).
fn labels() -> ValidateLabels {
    ValidateLabels {
        current_settings: texts::CURRENT_SETTINGS.to_owned(),
        candidate: texts::CANDIDATE.to_owned(),
        candidate_overall: texts::CANDIDATE_OVERALL.to_owned(),
        automatic: texts::AUTOMATIC.to_owned(),
        guards: texts::GUARDS.to_owned(),
        guards_limit: texts::GUARDS_LIMIT.to_owned(),
    }
}

/// A requirement that measured 1.5 °C against a limit of 2 °C at most, from the configured plant, with no reason.
fn requirement(id: &str, status: RequirementStatus) -> RequirementOutcome {
    RequirementOutcome {
        id: id.to_owned(),
        label: format!("Requirement {id}"),
        channel: RequirementChannel {
            role: "Measurement".to_owned(),
            unit: "degC".to_owned(),
            symbol: "TT101".to_owned(),
        },
        limit_side: LimitSide::AtMost,
        status,
        value: Some(1.5),
        limit: Some(2.0),
        unit: "degC".to_owned(),
        source: "Configured plant".to_owned(),
        reason: String::new(),
        automatic: false,
        guard: false,
        measured_range: None,
        limit_range: None,
    }
}

/// A check of a declared operating range that passed: measured between 1 and 1.5, inside the declared bounds 0 to 3.
fn passing_guard(id: &str) -> RequirementOutcome {
    RequirementOutcome {
        guard: true,
        measured_range: Some(ValueRange {
            minimum: 1.0,
            maximum: 1.5,
        }),
        limit_range: Some(ValueRange {
            minimum: 0.0,
            maximum: 3.0,
        }),
        ..requirement(id, RequirementStatus::Pass)
    }
}

/// A run of a scenario with the given requirements; the run's own status does not decide the table.
fn run(requirements: Vec<RequirementOutcome>) -> RunOutcome {
    RunOutcome {
        run_id: "run-1".to_owned(),
        project_revision: "rev-1".to_owned(),
        evidence: RunEvidence::StExecuted,
        status: RequirementStatus::Pass,
        requirements,
    }
}

/// A scenario whose current settings' run and candidate's run hold the given requirements.
fn scenario(
    label: &str,
    current: Vec<RequirementOutcome>,
    candidate: Vec<RequirementOutcome>,
) -> ScenarioOutcome {
    ScenarioOutcome {
        id: "target-response".to_owned(),
        plant: "Configured plant".to_owned(),
        label: label.to_owned(),
        current: run(current),
        candidate: run(candidate),
    }
}

/// The identities of the rows in their order: a requirement's id, or the marker of the guard row or of the overall row.
fn ids(table: &[ValidateRow]) -> Vec<&str> {
    table.iter().map(|row| row.requirement.as_str()).collect()
}

#[test]
fn a_passing_row_names_its_channel_and_shows_its_measured_limit_and_word() {
    let passing = requirement("settling", RequirementStatus::Pass);
    let table = rows(
        &labels(),
        None,
        &[scenario("Target response", vec![passing], Vec::new())],
        None,
        RequirementStatus::Pass,
    );
    assert_eq!(table.len(), 2);
    let row = &table[0];
    assert_eq!(
        row.title,
        "Current settings · Requirement settling · Measurement (°C)"
    );
    assert_eq!(row.symbol, "TT101");
    assert_eq!(row.source, "Configured plant");
    assert_eq!(row.measured, "1.5 °C");
    assert_eq!(row.limit, "≤ 2 °C");
    assert_eq!(row.verdict, "Pass");
    assert_eq!(row.verdict_text, "Pass");
    assert_eq!(row.verdict_class, "pass");
    assert_eq!(row.requirement, "settling");
}

#[test]
fn an_overall_row_closes_the_table_only_when_a_scenario_ran() {
    let model = requirement("model-check", RequirementStatus::Pass);
    let without = rows(&labels(), Some(&model), &[], None, RequirementStatus::Pass);
    assert_eq!(ids(&without), ["model-check"]);
    let with = rows(
        &labels(),
        Some(&model),
        &[scenario("Target response", Vec::new(), Vec::new())],
        None,
        RequirementStatus::Fail,
    );
    assert_eq!(ids(&with), ["model-check", "overall"]);
    assert_eq!(with[1].title, "Candidate overall");
    assert_eq!(with[1].measured, "—");
    assert_eq!(with[1].limit, "—");
    assert_eq!(with[1].verdict, "Fail");
    assert_eq!(with[1].verdict_text, "Fail");
}

#[test]
fn a_failing_row_shows_its_reason_after_its_word() {
    let failing = RequirementOutcome {
        reason: "The output settles at 2.4 °C, above the limit.".to_owned(),
        ..requirement("settling", RequirementStatus::Fail)
    };
    let table = rows(
        &labels(),
        None,
        &[scenario("Target response", vec![failing], Vec::new())],
        None,
        RequirementStatus::Fail,
    );
    assert_eq!(table[0].verdict, "Fail");
    assert_eq!(
        table[0].verdict_text,
        "Fail · The output settles at 2.4 °C, above the limit."
    );
    assert_eq!(table[0].verdict_class, "fail");
}

#[test]
fn an_inconclusive_row_has_its_class_and_its_reason_when_it_has_one() {
    let unknown = RequirementOutcome {
        reason: "No good sample lies in the window.".to_owned(),
        ..requirement("settling", RequirementStatus::Inconclusive)
    };
    let table = rows(
        &labels(),
        None,
        &[scenario("Target response", vec![unknown], Vec::new())],
        None,
        RequirementStatus::Inconclusive,
    );
    assert_eq!(table[0].verdict, "Inconclusive");
    assert_eq!(
        table[0].verdict_text,
        "Inconclusive · No good sample lies in the window."
    );
    assert_eq!(table[0].verdict_class, "inconclusive");
}

#[test]
fn rows_sort_by_verdict_with_the_model_check_first() {
    let model = requirement("model-check", RequirementStatus::Pass);
    let current = vec![
        requirement("passes", RequirementStatus::Pass),
        requirement("unknown", RequirementStatus::Inconclusive),
        requirement("fails", RequirementStatus::Fail),
    ];
    let candidate = vec![requirement("also-passes", RequirementStatus::Pass)];
    let scenarios = [scenario("Target response", current, candidate)];
    let table = rows(
        &labels(),
        Some(&model),
        &scenarios,
        None,
        RequirementStatus::Fail,
    );
    assert_eq!(
        ids(&table),
        [
            "model-check",
            "fails",
            "unknown",
            "passes",
            "also-passes",
            "overall"
        ]
    );
}

#[test]
fn passing_guards_are_counted_in_one_row_before_the_overall_row() {
    let current = vec![
        passing_guard("range-a"),
        passing_guard("range-b"),
        requirement("settling", RequirementStatus::Pass),
    ];
    let scenarios = [scenario("Target response", current, Vec::new())];
    let table = rows(&labels(), None, &scenarios, None, RequirementStatus::Pass);
    assert_eq!(
        ids(&table),
        ["settling", "limit-and-controller-checks", "overall"]
    );
    assert_eq!(table[1].title, "Ranges, states and transfer");
    assert_eq!(table[1].measured, "2 checks");
    assert_eq!(table[1].limit, "Within declared limits");
    assert_eq!(table[1].verdict, "Pass");
    assert_eq!(table[1].verdict_text, "Pass");
    assert_eq!(table[1].verdict_class, "pass");
}

#[test]
fn a_single_passing_guard_is_one_check() {
    let scenarios = [scenario(
        "Target response",
        vec![passing_guard("range")],
        Vec::new(),
    )];
    let table = rows(&labels(), None, &scenarios, None, RequirementStatus::Pass);
    assert_eq!(table[0].measured, "1 check");
}

#[test]
fn a_range_check_shows_its_measured_range_and_its_declared_bounds() {
    let range = RequirementOutcome {
        status: RequirementStatus::Fail,
        ..passing_guard("range")
    };
    let scenarios = [scenario("Target response", vec![range], Vec::new())];
    let table = rows(&labels(), None, &scenarios, None, RequirementStatus::Fail);
    assert_eq!(table[0].measured, "1–1.5 °C");
    assert_eq!(table[0].limit, "0–3 °C");
}

#[test]
fn a_single_declared_value_has_an_equal_sign_and_no_measured_range_is_a_dash() {
    let fixed = RequirementOutcome {
        limit_range: Some(ValueRange {
            minimum: 2.0,
            maximum: 2.0,
        }),
        ..requirement("fixed", RequirementStatus::Fail)
    };
    let scenarios = [scenario("Target response", vec![fixed], Vec::new())];
    let table = rows(&labels(), None, &scenarios, None, RequirementStatus::Fail);
    assert_eq!(table[0].limit, "= 2 °C");
    assert_eq!(table[0].measured, "—");
}

#[test]
fn a_one_sided_limit_names_its_side_and_a_missing_value_is_a_dash() {
    let share = RequirementOutcome {
        limit_side: LimitSide::AtLeast,
        limit: Some(5.0),
        unit: "1".to_owned(),
        value: None,
        ..requirement("share", RequirementStatus::Pass)
    };
    let scenarios = [scenario("Target response", vec![share], Vec::new())];
    let table = rows(&labels(), None, &scenarios, None, RequirementStatus::Pass);
    assert_eq!(table[0].limit, "≥ 5");
    assert_eq!(table[0].measured, "—");
}

#[test]
fn a_small_value_is_written_in_exponent_form_and_a_value_the_page_lacks_is_a_dash() {
    let small = RequirementOutcome {
        value: Some(0.0005),
        ..requirement("small", RequirementStatus::Pass)
    };
    let blank = RequirementOutcome {
        value: Some(f64::NAN),
        limit: Some(f64::INFINITY),
        ..requirement("blank", RequirementStatus::Inconclusive)
    };
    let scenarios = [scenario("Target response", vec![small, blank], Vec::new())];
    let table = rows(
        &labels(),
        None,
        &scenarios,
        None,
        RequirementStatus::Inconclusive,
    );
    // The inconclusive row comes first; the page receives a value that is not finite as null, which it shows as a dash.
    assert_eq!(table[0].requirement, "blank");
    assert_eq!(table[0].measured, "—");
    assert_eq!(table[0].limit, "—");
    assert_eq!(table[1].measured, "5.00e-4 °C");
}

#[test]
fn a_generated_limit_is_marked_automatic_between_its_label_and_its_channel() {
    let generated = RequirementOutcome {
        automatic: true,
        ..requirement("settling", RequirementStatus::Pass)
    };
    let scenarios = [scenario("Target response", vec![generated], Vec::new())];
    let table = rows(&labels(), None, &scenarios, None, RequirementStatus::Pass);
    assert_eq!(
        table[0].title,
        "Current settings · Requirement settling · Automatic · Measurement (°C)"
    );
}

#[test]
fn two_scenarios_put_their_label_and_their_run_in_front_of_each_row() {
    let first = scenario(
        "Target response",
        vec![requirement("a", RequirementStatus::Pass)],
        vec![requirement("b", RequirementStatus::Pass)],
    );
    let second = scenario(
        "Disturbance",
        Vec::new(),
        vec![requirement("c", RequirementStatus::Pass)],
    );
    let table = rows(
        &labels(),
        None,
        &[first, second],
        None,
        RequirementStatus::Pass,
    );
    let titles: Vec<&str> = table.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Target response · Current settings · Requirement a · Measurement (°C)",
            "Target response · Candidate · Requirement b · Measurement (°C)",
            "Disturbance · Candidate · Requirement c · Measurement (°C)",
            "Candidate overall",
        ]
    );
}

#[test]
fn the_manual_to_auto_rows_take_the_case_label() {
    let entry = RequirementOutcome {
        guard: true,
        ..requirement("entry", RequirementStatus::Fail)
    };
    let case = InitializationOutcome {
        label: "Manual-to-Auto entry".to_owned(),
        status: RequirementStatus::Fail,
        requirements: vec![entry],
    };
    let table = rows(&labels(), None, &[], Some(&case), RequirementStatus::Fail);
    assert_eq!(table.len(), 1);
    assert_eq!(
        table[0].title,
        "Manual-to-Auto entry · Requirement entry · Measurement (°C)"
    );
}

#[test]
fn a_failed_target_error_row_shows_the_observed_error_and_its_limit_in_the_loops_units() {
    // control-studio-verification-client: "explains a failed target using the actual observed error and its limit" and "names requirement
    // units the way the rest of Control Studio does" (B5.4, B5.5, B9.9). The row of a failed target error carries the observed error and
    // the limit with the unit in the page's notation (`degC` is `°C`), and the verdict cell keeps the requirement's own reason.
    for (observed, limit, measured_words, limit_words) in [
        (0.893_512_6, 0.1, "0.8935 °C", "≤ 0.1 °C"),
        (0.2053, 0.2, "0.2053 °C", "≤ 0.2 °C"),
    ] {
        let failing = RequirementOutcome {
            value: Some(observed),
            limit: Some(limit),
            reason: texts::measurement_exceeds_limit(),
            ..requirement("target-error", RequirementStatus::Fail)
        };
        let table = rows(
            &labels(),
            None,
            &[scenario("Target response", vec![failing], Vec::new())],
            None,
            RequirementStatus::Fail,
        );
        let row = &table[0];
        assert_eq!(row.requirement, "target-error");
        assert_eq!(row.measured, measured_words);
        assert_eq!(row.limit, limit_words);
        assert_eq!(row.verdict, "Fail");
        assert_eq!(
            row.verdict_text,
            "Fail · Observed measurement exceeds the declared limit."
        );
    }
}
