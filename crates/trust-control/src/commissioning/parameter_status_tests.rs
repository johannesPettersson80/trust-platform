use super::*;

fn values(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(symbol, value)| ((*symbol).to_owned(), *value))
        .collect()
}

fn status(active: &str, previous: Option<BTreeMap<String, f64>>) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: None,
        program_revision: "program:1".into(),
        binding_revision: "binding:1".into(),
        active_revision: active.into(),
        values: values(&[("Main.Kp", 2.0), ("Main.Ki", 0.5)]),
        status: RecordState::Applied,
        previous_revision: previous.as_ref().map(|_| "previous:1".to_owned()),
        previous_values: previous,
        reason: None,
    }
}

#[test]
fn parameter_values_are_the_same_only_when_every_parameter_holds_the_same_value() {
    let held = values(&[("Main.Kp", 2.0), ("Main.Ki", 0.5)]);
    assert!(same_parameter_values(&held, &held.clone()));
    assert!(
        !same_parameter_values(&held, &values(&[("Main.Kp", 2.0)])),
        "a missing parameter"
    );
    assert!(
        !same_parameter_values(&held, &values(&[("Main.Kp", 2.0), ("Main.Ki", 0.6)])),
        "a changed value"
    );
    assert!(
        !same_parameter_values(&held, &values(&[("Main.Kp", 2.0), ("Main.Kd", 0.5)])),
        "a parameter that is not in the record"
    );
}

#[test]
fn a_saved_status_is_the_history_only_while_revisions_and_values_match() {
    let applied = status(
        "applied:1",
        Some(values(&[("Main.Kp", 1.0), ("Main.Ki", 0.0)])),
    );
    assert!(
        !same_parameter_history(&applied, None),
        "no saved status is never the same history"
    );
    assert!(same_parameter_history(&applied, Some(&applied.clone())));

    let changed_active = status("applied:2", applied.previous_values.clone());
    assert!(
        !same_parameter_history(&changed_active, Some(&applied)),
        "a later active revision is another history"
    );

    let without_previous = status("applied:1", None);
    assert!(
        !same_parameter_history(&without_previous, Some(&applied)),
        "a record with and one without a previous set differ"
    );
    assert!(same_parameter_history(
        &without_previous,
        Some(&status("applied:1", None))
    ));
}
