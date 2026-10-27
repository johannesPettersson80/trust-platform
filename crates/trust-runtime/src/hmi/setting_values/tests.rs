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

/// The values of an enumeration none of these settings holds.
fn no_enumeration(_: &str) -> Option<Vec<(SmolStr, i64)>> {
    None
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
    let timeout = HmiSetting {
        title: "Start timeout".into(),
        ..setting("StartTimeout", Some("s"), Some(1.0), Some(600.0))
    };
    assert_eq!(
        requested_write(
            &timeout,
            timeout.unit.as_deref(),
            &seconds(5),
            &json!(10),
            no_enumeration
        ),
        Ok(WriteAction::Set(json!(10.0)))
    );
    let above = requested_write(
        &timeout,
        timeout.unit.as_deref(),
        &seconds(5),
        &json!(700),
        no_enumeration,
    )
    .expect_err("above its maximum");
    assert!(
        above.starts_with("Start timeout:") && above.contains("maximum 600 s"),
        "{above}"
    );
    assert!(requested_write(
        &timeout,
        timeout.unit.as_deref(),
        &seconds(5),
        &json!("soon"),
        no_enumeration
    )
    .is_err());
    let in_ms = setting("Delay", Some("ms"), None, None);
    assert_eq!(
        requested_write(
            &in_ms,
            in_ms.unit.as_deref(),
            &seconds(5),
            &json!(1500),
            no_enumeration
        ),
        Ok(WriteAction::Set(json!(1.5)))
    );
}

#[test]
fn a_choice_is_one_of_its_values_and_a_number_fits_its_type() {
    assert_eq!(
        requested_write(
            &per_hour(),
            per_hour().unit.as_deref(),
            &seconds(3600),
            &json!("T#1m"),
            no_enumeration
        ),
        Ok(WriteAction::Set(json!(60.0)))
    );
    assert!(requested_write(
        &per_hour(),
        per_hour().unit.as_deref(),
        &seconds(3600),
        &json!("T#1d"),
        no_enumeration
    )
    .is_err());
    let count = setting("Preset", None, None, None);
    assert!(
        requested_write(
            &count,
            count.unit.as_deref(),
            &Value::Int(0),
            &json!(40000),
            no_enumeration
        )
        .is_err_and(|reason| reason.contains("Preset")),
        "an INT holds no 40000"
    );
    assert!(requested_write(
        &count,
        count.unit.as_deref(),
        &Value::Int(0),
        &json!(1.5),
        no_enumeration
    )
    .is_err());
    let flag = setting("Heating", None, None, None);
    assert_eq!(
        requested_write(
            &flag,
            flag.unit.as_deref(),
            &Value::Bool(true),
            &json!(false),
            no_enumeration
        ),
        Ok(WriteAction::Set(json!(false)))
    );
    let interval = setting("ServiceInterval", Some("h"), Some(0.0), Some(100000.0));
    assert!(
        requested_write(
            &interval,
            interval.unit.as_deref(),
            &Value::LReal(4000.0),
            &json!(-1),
            no_enumeration
        )
        .is_err(),
        "below its minimum"
    );
}

/// The Settings dialog sends each entry as its field holds it (§21.4 item 4): a number as text.
/// "1,5" reads as 1.5 (a comma is the decimal separator), " 2.25 " as 2.25 (trimmed); "abc" and an
/// empty field are no number, refused naming the setting. Catches a comma not read as the decimal
/// separator, a text written as text to the LREAL, and a text that is no number accepted.
#[test]
fn a_number_entered_as_text_is_read_with_a_comma_as_its_decimal_separator() {
    let gain = setting("Gain", None, None, None);
    let write = |entry: &str| {
        requested_write(
            &gain,
            gain.unit.as_deref(),
            &Value::LReal(1.0),
            &json!(entry),
            no_enumeration,
        )
    };
    assert_eq!(write("1,5"), Ok(WriteAction::Set(json!(1.5))));
    assert_eq!(write(" 2.25 "), Ok(WriteAction::Set(json!(2.25))));
    assert_eq!(
        write("abc"),
        Err("Gain: enter a number, not \"abc\"".to_string())
    );
    assert_eq!(write(""), Err("Gain: enter a number, not \"\"".to_string()));
}

/// An integer setting takes a whole number (§21.4 item 4): "40" is 40; "2,5" and "2.5" are 2.5,
/// refused as no whole number; "40000" is whole but more than an INT holds (32 767). Catches a
/// fraction cut or rounded to 2 and written, and a text entry that skips the type's capacity.
#[test]
fn an_integer_setting_takes_a_whole_number_entered_as_text() {
    let preset = setting("Preset", None, None, None);
    let write = |entry: &str| {
        requested_write(
            &preset,
            preset.unit.as_deref(),
            &Value::Int(0),
            &json!(entry),
            no_enumeration,
        )
    };
    assert_eq!(write("40"), Ok(WriteAction::Set(json!(40))));
    for fraction in ["2,5", "2.5"] {
        assert_eq!(
            write(fraction),
            Err("Preset: enter a whole number, not 2.5".to_string()),
            "{fraction}"
        );
    }
    assert!(
        write("40000")
            .is_err_and(|reason| reason.starts_with("Preset: ") && reason.contains("40000")),
        "an INT holds no 40000"
    );
}

/// A number entered as text is in the setting's unit and within its limits (§21.4 item 4; Plant
/// §10): for the pump's start timeout in s from 1 to 600, "2,5" is 2.5 s, "0,5" is below the
/// minimum 1 s and "700" above the maximum 600 s; a delay in min takes "1,5" as 90 s; the service
/// interval in h from 0 refuses "-1". Catches a text duration read in seconds instead of its unit
/// (1.5 s), limits not applied to a text entry, and a refusal without the setting's title or unit.
#[test]
fn a_number_entered_as_text_is_checked_against_the_limits_in_its_unit() {
    let timeout = HmiSetting {
        title: "Start timeout".into(),
        ..setting("StartTimeout", Some("s"), Some(1.0), Some(600.0))
    };
    let write = |entry: &str| {
        requested_write(
            &timeout,
            timeout.unit.as_deref(),
            &seconds(5),
            &json!(entry),
            no_enumeration,
        )
    };
    assert_eq!(write("2,5"), Ok(WriteAction::Set(json!(2.5))));
    assert_eq!(
        write("0,5"),
        Err("Start timeout: 0.5 is below the minimum 1 s".to_string())
    );
    assert_eq!(
        write("700"),
        Err("Start timeout: 700 is above the maximum 600 s".to_string())
    );
    let delay = setting("Delay", Some("min"), None, None);
    assert_eq!(
        requested_write(
            &delay,
            delay.unit.as_deref(),
            &seconds(60),
            &json!("1,5"),
            no_enumeration
        ),
        Ok(WriteAction::Set(json!(90.0)))
    );
    let interval = HmiSetting {
        title: "Service interval".into(),
        ..setting("ServiceInterval", Some("h"), Some(0.0), Some(100000.0))
    };
    assert_eq!(
        requested_write(
            &interval,
            interval.unit.as_deref(),
            &Value::LReal(4000.0),
            &json!("-1"),
            no_enumeration
        ),
        Err("Service interval: -1 is below the minimum 0 h".to_string())
    );
}

/// A list of choices sends the chosen value's text and a checkbox TRUE or FALSE (§21.4 item 4):
/// "2" names the choice 2 and "3" none; "T#1h" the hour, 3 600 s; for a flag "false" and " TRUE "
/// are FALSE and TRUE, and "maybe" is neither. Catches a choice's value sent as text not found (no
/// number choice could be applied), and any other text read as FALSE and written.
#[test]
fn a_choice_is_named_by_its_values_text_and_a_flag_by_true_or_false() {
    let speed = HmiSetting {
        choices: vec![
            HmiChoice {
                value: HmiScalar::Number(1.0),
                title: "slow".into(),
            },
            HmiChoice {
                value: HmiScalar::Number(2.0),
                title: "fast".into(),
            },
        ],
        ..setting("Speed", None, None, None)
    };
    assert_eq!(
        requested_write(
            &speed,
            speed.unit.as_deref(),
            &Value::Int(1),
            &json!("2"),
            no_enumeration
        ),
        Ok(WriteAction::Set(json!(2.0)))
    );
    assert_eq!(
        requested_write(
            &speed,
            speed.unit.as_deref(),
            &Value::Int(1),
            &json!("3"),
            no_enumeration
        ),
        Err("Speed: choose one of its values".to_string())
    );
    assert_eq!(
        requested_write(
            &per_hour(),
            per_hour().unit.as_deref(),
            &seconds(60),
            &json!("T#1h"),
            no_enumeration
        ),
        Ok(WriteAction::Set(json!(3600.0)))
    );
    let flag = setting("Heating", None, None, None);
    let write = |entry: &str| {
        requested_write(
            &flag,
            flag.unit.as_deref(),
            &Value::Bool(true),
            &json!(entry),
            no_enumeration,
        )
    };
    assert_eq!(write("false"), Ok(WriteAction::Set(json!(false))));
    assert_eq!(write(" TRUE "), Ok(WriteAction::Set(json!(true))));
    assert!(
        write("maybe").is_err_and(|reason| reason.starts_with("Heating: ")),
        "neither TRUE nor FALSE"
    );
}

/// Specification 39 §21.4: a refusal states a limit in the unit the dialog shows, a `$Member` unit
/// resolved to its member's text (`m` here). Catches the unit dropped from the refusal, which then
/// read `100` where the dialog shows `100 m`.
#[test]
fn a_limit_refusal_states_the_unit_the_dialog_shows() {
    let range = setting("RangeHigh", Some("$Unit"), Some(0.0), Some(100.0));
    let refused = requested_write(&range, Some("m"), &Value::LReal(50.0), &json!(150), |_| {
        None
    })
    .expect_err("above the maximum");
    assert_eq!(refused, "RangeHigh: 150 is above the maximum 100 m");
    let unresolved = requested_write(
        &range,
        Some("$Unit"),
        &Value::LReal(50.0),
        &json!(150),
        |_| None,
    )
    .expect_err("above the maximum");
    assert_eq!(unresolved, "RangeHigh: 150 is above the maximum 100");
}
