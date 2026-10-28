//! Tests of the cascade's Tune request (specification 35, C6): Automatic designs both PIs, a PI is the only structure, and no setting is
//! pinned. The design itself is tested on the model in `tuning::structures`, and the candidate and its answer in their own modules.

use super::*;
use crate::messages::TuneMethod;

fn request(method: Option<TuneMethod>, structure: Option<Structure>) -> TuneRequest {
    TuneRequest {
        project_root: "/projects/vessel".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "vessel-temperature".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method,
        choice: None,
        structure,
        pins: None,
        settings: None,
        connection: None,
    }
}

#[test]
fn a_named_method_is_refused_because_both_pis_are_designed_by_automatic() {
    let refusal = check_request(&request(Some(TuneMethod::Simc), None))
        .expect_err("a named method does not design a cascade");
    assert_eq!(refusal.message, AUTOMATIC);
    assert_eq!(refusal.next_action, NextAction::ChooseMethod);
}

#[test]
fn a_structure_other_than_pi_is_refused_because_both_controllers_are_pi() {
    let refusal = check_request(&request(None, Some(Structure::PID)))
        .expect_err("a cascade has two PI controllers");
    assert_eq!(refusal.message, STRUCTURE);
}

#[test]
fn automatic_with_pi_or_no_structure_is_accepted() {
    assert!(check_request(&request(None, Some(Structure::PI))).is_ok());
    assert!(check_request(&request(None, None)).is_ok());
}
