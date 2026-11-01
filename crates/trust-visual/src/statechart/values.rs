//! The values the Statechart editor's panels read from typed text (specification 25 §11.4): a
//! transition's auto-transition timer, an action mapping's value, and the name of a new action
//! mapping. The panels send the text as typed and store the value answered here.

use crate::js;
use crate::Json;

/// The message when the timer field holds no delay.
const AFTER_MESSAGE: &str =
    "Auto-transition timer must be a number of milliseconds from 0, or empty for none.";
/// The message when the new action's name is blank.
const NAME_MESSAGE: &str = "Enter an action name.";
/// The message when a mapping already has the new action's name.
const NAME_IN_USE_MESSAGE: &str = "An action with this name already exists.";

/// The `after` delay of a transition for the text typed into its timer field: none for blank
/// text, so the transition waits for its event; otherwise the milliseconds the text reads as
/// (`Number(text)`, [`js::number_from_text`]) without their fraction, as the field's `parseInt`
/// kept whole milliseconds.
///
/// # Errors
/// The text is not a number, not finite or below 0; the transition keeps its delay then.
pub fn after_delay(text: &str) -> Result<Option<f64>, String> {
    if js::trim(text).is_empty() {
        return Ok(None);
    }
    match js::number_from_text(text) {
        Some(milliseconds) if milliseconds.is_finite() && milliseconds >= 0.0 => {
            Ok(Some(milliseconds.trunc() + 0.0))
        }
        _ => Err(AFTER_MESSAGE.to_owned()),
    }
}

/// The value of an action mapping for the text typed into its value field: `true` and `false`
/// are Booleans, text that reads as a finite number ([`js::number_from_text`]) is that number, and
/// any other text, blank text included, stays text.
#[must_use]
pub fn mapping_value(text: &str) -> Json {
    match text {
        "true" => Json::Bool(true),
        "false" => Json::Bool(false),
        _ if js::trim(text).is_empty() => Json::str(text),
        _ => match js::number_from_text(text) {
            Some(number) if number.is_finite() => Json::Number(number + 0.0),
            _ => Json::str(text),
        },
    }
}

/// The action mappings `mappings` with a new mapping under `name`, the text typed into the Add
/// dialog without its surrounding blanks, and that name. The new action writes `TRUE` to
/// `%QX0.0`, the mapping the panel gives a new action, which the user then edits; it comes after
/// the others.
///
/// # Errors
/// The name is blank, or a mapping already has it; the mappings stay as they are then.
pub fn add_mapping(mappings: &Json, name: &str) -> Result<(Json, String), String> {
    let name = js::trim(name);
    if name.is_empty() {
        return Err(NAME_MESSAGE.to_owned());
    }
    if mappings.get(name).is_some() {
        return Err(NAME_IN_USE_MESSAGE.to_owned());
    }
    let mut next = match mappings {
        Json::Object(_) => mappings.clone(),
        _ => Json::object(),
    };
    let mut mapping = Json::object();
    mapping.set("action", Json::str("WRITE_OUTPUT"));
    mapping.set("address", Json::str("%QX0.0"));
    mapping.set("value", Json::Bool(true));
    next.set(name, mapping);
    Ok((next, name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_action_writes_true_to_the_first_output_after_the_others() {
        // Specification 25 §11.4: the name without its blanks, the panel's mapping for a new
        // action, after the existing ones, which stay as they were. Catches a name stored with its
        // blanks and an existing mapping replaced or reordered.
        let mappings = Json::parse(
            r#"{"START_MOTOR":{"action":"WRITE_OUTPUT","address":"%QX0.1","value":true}}"#,
        )
        .expect("mappings");
        let (next, name) = add_mapping(&mappings, "  STOP_MOTOR ").expect("added");
        assert_eq!(name, "STOP_MOTOR");
        assert_eq!(
            next.to_compact(),
            r#"{"START_MOTOR":{"action":"WRITE_OUTPUT","address":"%QX0.1","value":true},"STOP_MOTOR":{"action":"WRITE_OUTPUT","address":"%QX0.0","value":true}}"#
        );
    }

    #[test]
    fn a_blank_name_or_one_in_use_adds_nothing() {
        // The panel's check before: blank names were dropped and a name in use was refused.
        // Catches a second mapping under one name, which replaced the first one's action.
        let mappings =
            Json::parse(r#"{"START_MOTOR":{"action":"WRITE_OUTPUT"}}"#).expect("mappings");
        assert_eq!(add_mapping(&mappings, "   "), Err(NAME_MESSAGE.to_owned()));
        assert_eq!(add_mapping(&mappings, ""), Err(NAME_MESSAGE.to_owned()));
        assert_eq!(
            add_mapping(&mappings, " START_MOTOR "),
            Err(NAME_IN_USE_MESSAGE.to_owned())
        );
    }

    #[test]
    fn a_blank_timer_removes_the_delay_and_a_number_is_whole_milliseconds() {
        // Worked by hand: blank text is no delay; 12.9 keeps its 12 whole milliseconds, as the
        // field's parseInt kept them; 1e3 is 1000 (parseInt read it as 1); -0 is 0. Catches a
        // blank field that stores 0 (a transition firing at once) or an exponent read as its
        // leading digit.
        assert_eq!(after_delay(""), Ok(None));
        assert_eq!(after_delay("  "), Ok(None));
        assert_eq!(after_delay("1000"), Ok(Some(1000.0)));
        assert_eq!(after_delay(" 250 "), Ok(Some(250.0)));
        assert_eq!(after_delay("12.9"), Ok(Some(12.0)));
        assert_eq!(after_delay("1e3"), Ok(Some(1000.0)));
        assert_eq!(after_delay("0"), Ok(Some(0.0)));
        assert_eq!(
            after_delay("-0").map(|after| after.map(|ms| Json::Number(ms).to_compact())),
            Ok(Some("0".to_owned()))
        );
    }

    #[test]
    fn a_timer_that_is_no_delay_is_refused() {
        // A negative delay was ignored without a word; text and infinity are no delay either.
        // Catches a field that keeps showing a value the transition does not have.
        for text in ["-5", "-0.5", "abc", "12abc", "Infinity", "1e400"] {
            assert_eq!(after_delay(text), Err(AFTER_MESSAGE.to_owned()), "{text:?}");
        }
    }

    #[test]
    fn a_mapping_value_is_a_boolean_a_number_or_text() {
        // Worked by hand from the panel's rule: exactly `true`/`false` are Booleans (`TRUE` stays
        // text, as before), finite numbers are numbers (blanks around them and `0x1F` included),
        // everything else is text. Catches a value written as the wrong ST literal by the
        // companion (`'42'` for 42, `'true'` for TRUE).
        assert_eq!(mapping_value("true"), Json::Bool(true));
        assert_eq!(mapping_value("false"), Json::Bool(false));
        assert_eq!(mapping_value("TRUE"), Json::str("TRUE"));
        assert_eq!(mapping_value("42"), Json::Number(42.0));
        assert_eq!(mapping_value(" 42 "), Json::Number(42.0));
        assert_eq!(mapping_value("-1.5"), Json::Number(-1.5));
        assert_eq!(mapping_value("0x1F"), Json::Number(31.0));
        assert_eq!(mapping_value("hello"), Json::str("hello"));
        assert_eq!(mapping_value("'hello'"), Json::str("'hello'"));
        assert_eq!(mapping_value(""), Json::str(""));
    }

    #[test]
    fn blank_text_and_infinity_stay_text() {
        // `Number(" ")` is 0 and `Number("Infinity")` is infinity, which the file wrote as null;
        // both were stored without being typed as numbers. Catches a value of 0 or null that
        // nobody typed.
        assert_eq!(mapping_value(" "), Json::str(" "));
        assert_eq!(mapping_value("Infinity"), Json::str("Infinity"));
        assert_eq!(mapping_value("-Infinity"), Json::str("-Infinity"));
        assert_eq!(mapping_value("1e400"), Json::str("1e400"));
    }
}
