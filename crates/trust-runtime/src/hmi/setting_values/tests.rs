//! Specification 39, §21.4 and Plant §10: settings are shown in their unit and every value an
//! Engineer applies is checked against the component definition and the variable's type.

use super::*;
use crate::value::Duration;

fn setting(member: &str, unit: Option<&str>, min: Option<f64>, max: Option<f64>) -> HmiSetting {
    HmiSetting {
        member: member.into(),
        title: member.into(),
        unit: unit.map(str::to_string),
        min,
        max,
        step: None,
        choices: Vec::new(),
        key: false,
        help: None,
    }
}

fn seconds(count: i64) -> Value {
    Value::Time(Duration::from_secs(count))
}

fn per_hour() -> HmiSetting {
    HmiSetting {
        choices: vec![
            HmiChoice {
                value: HmiScalar::Text("T#1m".into()),
                title: "minute".into(),
            },
            HmiChoice {
                value: HmiScalar::Text("T#1h".into()),
                title: "hour".into(),
            },
        ],
        ..setting("TotalPer", None, None, None)
    }
}

#[test]
fn a_duration_is_shown_as_a_number_in_its_unit_seconds_by_default() {
    let timeout = setting("StartTimeout", None, Some(1.0), Some(600.0));
    let shown = shown_setting(&timeout, None, Some(&seconds(5)));
    assert_eq!(shown.kind, SettingKind::Time);
    assert_eq!(shown.unit.as_deref(), Some("s"));
    assert_eq!(shown.value.as_f64(), Some(5.0));
    let in_ms = setting("StartTimeout", Some("ms"), None, None);
    assert_eq!(shown_value(&in_ms, &seconds(5)).as_f64(), Some(5000.0));
    let in_minutes = setting("Delay", Some("min"), None, None);
    assert_eq!(shown_value(&in_minutes, &seconds(90)).as_f64(), Some(1.5));
}

#[test]
fn a_value_a_choice_holds_is_that_choices_value_and_a_member_unit_is_resolved() {
    let shown = shown_setting(&per_hour(), None, Some(&seconds(3600)));
    assert_eq!(shown.value, json!("T#1h"));
    assert_eq!(
        shown.unit, None,
        "a duration with choices is chosen by title"
    );
    let range = setting("RangeHigh", Some("$Unit"), None, None);
    let shown = shown_setting(&range, Some("m³/h".into()), Some(&Value::LReal(120.0)));
    assert_eq!(
        (shown.unit.as_deref(), shown.kind, shown.value.as_f64()),
        (Some("m³/h"), SettingKind::Real, Some(120.0))
    );
    assert_eq!(
        shown_setting(&range, None, None).value,
        serde_json::Value::Null
    );
}

#[test]
fn a_duration_is_written_in_seconds_from_its_unit_and_its_limits_name_the_setting() {
    let registry = TypeRegistry::new();
    let timeout = HmiSetting {
        title: "Start timeout".into(),
        ..setting("StartTimeout", Some("s"), Some(1.0), Some(600.0))
    };
    assert_eq!(
        requested_write(&registry, &timeout, &seconds(5), &json!(10)),
        Ok(WriteAction::Set(json!(10.0)))
    );
    let above = requested_write(&registry, &timeout, &seconds(5), &json!(700))
        .expect_err("above its maximum");
    assert!(
        above.starts_with("Start timeout:") && above.contains("maximum 600 s"),
        "{above}"
    );
    assert!(requested_write(&registry, &timeout, &seconds(5), &json!("soon")).is_err());
    let in_ms = setting("Delay", Some("ms"), None, None);
    assert_eq!(
        requested_write(&registry, &in_ms, &seconds(5), &json!(1500)),
        Ok(WriteAction::Set(json!(1.5)))
    );
}

#[test]
fn a_choice_is_one_of_its_values_and_a_number_fits_its_type() {
    let registry = TypeRegistry::new();
    assert_eq!(
        requested_write(&registry, &per_hour(), &seconds(3600), &json!("T#1m")),
        Ok(WriteAction::Set(json!(60.0)))
    );
    assert!(requested_write(&registry, &per_hour(), &seconds(3600), &json!("T#1d")).is_err());
    let count = setting("Preset", None, None, None);
    assert!(
        requested_write(&registry, &count, &Value::Int(0), &json!(40000))
            .is_err_and(|reason| reason.contains("Preset")),
        "an INT holds no 40000"
    );
    assert!(requested_write(&registry, &count, &Value::Int(0), &json!(1.5)).is_err());
    let flag = setting("Heating", None, None, None);
    assert_eq!(
        requested_write(&registry, &flag, &Value::Bool(true), &json!(false)),
        Ok(WriteAction::Set(json!(false)))
    );
    let interval = setting("ServiceInterval", Some("h"), Some(0.0), Some(100000.0));
    assert!(
        requested_write(&registry, &interval, &Value::LReal(4000.0), &json!(-1)).is_err(),
        "below its minimum"
    );
}
