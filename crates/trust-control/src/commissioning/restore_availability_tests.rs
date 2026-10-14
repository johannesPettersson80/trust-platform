use std::collections::BTreeMap;

use super::*;
use crate::commissioning::parameter_status::RecordState;

fn applied(active: &str) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("apply-1".into()),
        program_revision: "program:1".into(),
        binding_revision: "binding:1".into(),
        active_revision: active.into(),
        values: BTreeMap::from([("Main.Kp".to_owned(), 2.0)]),
        status: RecordState::Applied,
        previous_revision: Some("original:1".into()),
        previous_values: Some(BTreeMap::from([("Main.Kp".to_owned(), 1.0)])),
        reason: None,
    }
}

#[test]
fn restore_proceeds_while_the_settings_apply_wrote_are_still_the_active_ones() {
    let record = applied("applied:1");
    assert_eq!(restore_unavailable_reason(&record, Some(&record)), None);
    assert_eq!(
        restore_unavailable_reason(&record, None),
        None,
        "without a retained Apply there is no other change to overwrite"
    );
}

#[test]
fn restore_is_unavailable_once_the_active_revision_changed_and_overwrites_nothing() {
    let saved = applied("applied:1");
    let changed = applied("applied:2");
    assert_eq!(
        restore_unavailable_reason(&changed, Some(&saved)).as_deref(),
        Some(
            "Restore is unavailable: the controller settings or program changed after Apply, so restoring would overwrite another change. Nothing was written."
        ),
        "the `TypeScript` sentence, verbatim"
    );
}
