//! What a control declares in a page file (specification 39, §5.1, §5.8, §12.8): its role,
//! confirmation question, button action, options, pulse, expected feedback and timeout.

use std::time::Duration;

use super::load::{error_at, line_at, PageFileError, RawObject};
use super::types::{HmiButtonAction, HmiRole};

/// The longest pulse or confirmation time a control may declare (specification 39, §5): ten
/// minutes, longer than any pulse or confirmation in PLC practice.
pub const MAX_CONTROL_TIME: Duration = Duration::from_secs(600);

/// `seconds` as a control's `pulse` or `timeout`: positive and at most [`MAX_CONTROL_TIME`].
/// `None` for any other number, zero, a negative number, an infinity and NaN among them.
#[must_use]
pub fn control_time(seconds: f64) -> Option<Duration> {
    (seconds > 0.0 && seconds <= MAX_CONTROL_TIME.as_secs_f64())
        .then(|| Duration::from_secs_f64(seconds))
}

/// Why a control's `key` (`pulse` or `timeout`) holds no control time.
#[must_use]
pub fn control_time_error(key: &str) -> String {
    format!(
        "{key} must be a positive number of seconds, at most {}",
        MAX_CONTROL_TIME.as_secs()
    )
}

/// A role a control may ask for.
pub(super) fn role(text: &str) -> Option<HmiRole> {
    match text {
        "operator" => Some(HmiRole::Operator),
        "engineer" => Some(HmiRole::Engineer),
        _ => None,
    }
}

/// What a button may do.
pub(super) fn action(text: &str) -> Option<HmiButtonAction> {
    match text {
        "set" => Some(HmiButtonAction::Set),
        "reset" => Some(HmiButtonAction::Reset),
        "toggle" => Some(HmiButtonAction::Toggle),
        _ => None,
    }
}

/// Every control key is valid; each problem is reported on its line.
pub(super) fn check(
    file: &str,
    text: &str,
    objects: &[RawObject],
) -> Result<(), Vec<PageFileError>> {
    let mut errors = Vec::new();
    let mut report = |offset: usize, message: String| {
        errors.push(error_at(file, line_at(text, offset), message))
    };
    for object in objects {
        if let Some(command) = &object.command {
            if let Err(message) = command.validate() {
                report(object.id.span().start, message);
            }
            if object.action.is_some() || object.pulse.is_some() {
                report(
                    object.id.span().start,
                    "a capability command cannot also declare a scalar action or pulse".into(),
                );
            }
        }
        if let Some(value) = &object.role
            && role(value.get_ref()).is_none()
        {
            report(
                value.span().start,
                format!(
                    "role must be operator or engineer, not \"{}\"",
                    value.get_ref()
                ),
            );
        }
        if let Some(value) = &object.action
            && action(value.get_ref()).is_none()
        {
            report(
                value.span().start,
                format!(
                    "action must be set, reset or toggle, not \"{}\"",
                    value.get_ref()
                ),
            );
        }
        if let Some(value) = &object.confirm
            && value.get_ref().trim().is_empty()
        {
            report(
                value.span().start,
                "confirm must ask a question".to_string(),
            );
        }
        check_options(object, &mut report);
        for (key, value) in [("pulse", &object.pulse), ("timeout", &object.timeout)] {
            if let Some(value) = value
                && control_time(*value.get_ref()).is_none()
            {
                report(value.span().start, control_time_error(key));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// A selector's and a mode selector's choices (§5.8): at least one, each once, and for a mode
/// selector only Manual, Auto and Track. A selector needs them; a switch may have the words of its
/// two states, off first (`["Auto", "Manual"]`); other objects have none.
fn check_options(object: &RawObject, report: &mut impl FnMut(usize, String)) {
    let chooses = matches!(
        object.kind.as_str(),
        "selector" | "mode-selector" | "switch"
    );
    let Some(options) = &object.options else {
        if object.kind == "selector" {
            report(
                object.id.span().start,
                format!("the selector \"{}\" needs options", object.id.get_ref()),
            );
        }
        return;
    };
    if !chooses {
        report(
            options.span().start,
            "options belongs to selectors, mode selectors and switches".to_string(),
        );
        return;
    }
    if object.kind == "switch" && options.get_ref().len() != 2 {
        report(
            options.span().start,
            "a switch's options are the words of its two states, off first".to_string(),
        );
        return;
    }
    if options.get_ref().is_empty() {
        report(
            options.span().start,
            "options must list at least one choice".to_string(),
        );
    }
    let mut seen: Vec<String> = Vec::new();
    for option in options.get_ref() {
        let text = option.get_ref();
        if text.trim().is_empty() {
            report(option.span().start, "an option must have text".to_string());
        } else if seen.contains(&text.to_ascii_lowercase()) {
            report(option.span().start, format!("\"{text}\" is listed twice"));
        }
        seen.push(text.to_ascii_lowercase());
    }
}
