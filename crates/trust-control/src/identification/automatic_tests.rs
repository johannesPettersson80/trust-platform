//! Tests of the automatic identification records and the noise a requirement reads (`automaticRequirementEvidence.ts`).

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.
#![allow(clippy::float_cmp)] // The baseline noise of a flat or empty record is exactly zero.

use super::{baseline_noise, recorded_values, CampaignStatus, IdentificationCampaignRecord};
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::record_fixtures::{setpoint_record, Row, Verdict};

const CHANNELS: &[(&str, &str)] = &[("MV", "%"), ("PV", "degC")];

#[test]
fn recorded_values_keep_only_good_finite_values_in_time_order() {
    let rows: Vec<Row> = vec![
        (0.0, 0, vec![Some(50.0), Some(1.0)]),
        (0.1, 1, vec![Some(50.0), None]),
        (0.2, 2, vec![Some(50.0), Some(3.0)]),
    ];
    let record = setpoint_record("values", CHANNELS, &rows, RequirementStatus::Pass);
    assert_eq!(recorded_values(Some(&record), "PV"), vec![1.0, 3.0]);
    assert_eq!(recorded_values(Some(&record), "NOPE"), Vec::<f64>::new());
    assert_eq!(recorded_values::<Verdict>(None, "PV"), Vec::<f64>::new());
}

#[test]
fn the_baseline_noise_is_three_sigma_about_a_trend_before_the_input_changes() {
    // Ten outputs alternate ±0.1 about 5 before the input steps at the eleventh sample. Their least-squares residual
    // sum of squares is 16/165, over n − 2 = 8 degrees of freedom, so the noise is 3·√(2/165).
    let rows: Vec<Row> = (0..12_u64)
        .map(|index| {
            let input = if index < 10 { 50.0 } else { 60.0 };
            let output = if index >= 10 {
                7.0
            } else if index % 2 == 0 {
                5.1
            } else {
                4.9
            };
            (index as f64 * 0.1, index, vec![Some(input), Some(output)])
        })
        .collect();
    let record = setpoint_record("baseline", CHANNELS, &rows, RequirementStatus::Pass);
    let noise = baseline_noise(Some(&record), &["MV"], "PV");
    let expected = 3.0 * (2.0_f64 / 165.0).sqrt();
    assert!(
        (noise - expected).abs() <= 1e-12,
        "noise {noise} expected {expected}"
    );
}

#[test]
fn no_baseline_noise_without_an_input_change_or_enough_samples() {
    let steady: Vec<Row> = (0..12_u64)
        .map(|index| {
            (
                index as f64 * 0.1,
                index,
                vec![Some(50.0), Some(5.0 + index as f64 * 0.01)],
            )
        })
        .collect();
    let record = setpoint_record("steady", CHANNELS, &steady, RequirementStatus::Pass);
    assert_eq!(baseline_noise(Some(&record), &["MV"], "PV"), 0.0);
    let short: Vec<Row> = (0..7_u64)
        .map(|index| (index as f64 * 0.1, index, vec![Some(50.0), Some(5.0)]))
        .collect();
    let short_record = setpoint_record("short", CHANNELS, &short, RequirementStatus::Pass);
    assert_eq!(baseline_noise(Some(&short_record), &["MV"], "PV"), 0.0);
    assert_eq!(baseline_noise::<Verdict>(None, &["MV"], "PV"), 0.0);
}

#[test]
fn a_campaign_record_names_its_status_in_lower_case() {
    let status: CampaignStatus =
        serde_json::from_value(serde_json::json!("cancelled")).expect("a status word");
    assert_eq!(status, CampaignStatus::Cancelled);
    let record: IdentificationCampaignRecord<Verdict> = IdentificationCampaignRecord {
        status: CampaignStatus::Running,
        reason: None,
        runs: Vec::new(),
    };
    let text = serde_json::to_value(&record).expect("the record encodes");
    assert_eq!(text["status"], "running");
    assert!(
        text.get("reason").is_none(),
        "an absent reason is not written"
    );
}
