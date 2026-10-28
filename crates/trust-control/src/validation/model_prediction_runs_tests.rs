use super::*;
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::record_fixtures::{setpoint_record, Row, Verdict};

fn prediction(id: &str) -> RuntimeVerificationResult<Verdict> {
    let rows: [Row; 2] = [
        (0.0, 1, vec![Some(0.0), Some(1.0)]),
        (0.1, 2, vec![Some(1.0), Some(2.0)]),
    ];
    setpoint_record(
        id,
        &[("MV", "%"), ("PV", "degC")],
        &rows,
        RequirementStatus::Pass,
    )
}

#[test]
fn the_candidate_is_the_first_case_of_the_campaign() {
    // The campaign's first case is the nominal run, `target-response` (B2).
    assert_eq!(
        candidate_case(&["target-response", "load-rejection"]),
        Some(&"target-response")
    );
    assert_eq!(candidate_case::<&str>(&[]), None);
}

#[test]
fn baseline_and_candidate_predictions_compare_only_on_one_project_revision() {
    let baseline = prediction("baseline");
    let candidate = prediction("candidate");
    assert_eq!(same_project_revision(&baseline, &candidate), Ok(()));
    let mut changed = prediction("candidate");
    changed.project_revision = "sha256:changed".to_owned();
    assert_eq!(
        same_project_revision(&baseline, &changed),
        Err(Failure::new(
            &texts::project_changed_during_prediction(),
            Cause::Runtime,
            NextAction::Validate
        ))
    );
}
