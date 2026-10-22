//! The words of a duration (specification 35, B9.9, "Durations"): a number of seconds written in the largest fitting unit, as the engine
//! answers a duration that the page shows. The reader is `duration_seconds_of_text` of `loop_bindings`; the words read back through it as
//! the same duration, so the page shows the engine's text, sends back what the engineer typed, and keeps no formatter whose output the
//! engine has to parse.

use super::loop_bindings::duration_seconds_of_text;

/// The seconds from which a duration is written in minutes: two minutes.
const MINUTES_FROM: f64 = 120.0;
/// The seconds from which a duration is written in hours: two hours.
const HOURS_FROM: f64 = 7_200.0;

/// The words of `seconds` (B9.9): seconds below 2 min, minutes below 2 h, then hours and minutes, with a remainder written in the next
/// smaller unit, such as `90 s`, `15 min`, `2 min 5 s`, `75 min`, `2 h` and `1 h 30 min`. A number is written in plain decimals, never
/// with an exponent, which the reader refuses. The words always read back as `seconds`: where the units would not add up to exactly the
/// same number, the plain seconds are written instead.
#[must_use]
pub fn duration_words(seconds: f64) -> String {
    let composed = composed_words(seconds);
    if duration_seconds_of_text(&composed) == Some(seconds) {
        composed
    } else {
        plain_seconds(seconds)
    }
}

/// The unit-by-unit words of `seconds`.
fn composed_words(seconds: f64) -> String {
    if !(seconds.is_finite() && seconds >= MINUTES_FROM) {
        return plain_seconds(seconds);
    }
    if seconds < HOURS_FROM {
        let minutes = (seconds / 60.0).floor();
        let rest = seconds - minutes * 60.0;
        return if rest == 0.0 {
            format!("{minutes} min")
        } else {
            format!("{minutes} min {}", plain_seconds(rest))
        };
    }
    let hours = (seconds / 3_600.0).floor();
    let rest_of_hour = seconds - hours * 3_600.0;
    let minutes = (rest_of_hour / 60.0).floor();
    let rest = rest_of_hour - minutes * 60.0;
    let mut parts = vec![format!("{hours} h")];
    if minutes != 0.0 {
        parts.push(format!("{minutes} min"));
    }
    if rest != 0.0 {
        parts.push(plain_seconds(rest));
    }
    parts.join(" ")
}

/// A number of seconds in plain decimals with its unit: Rust writes a float without an exponent, as `0.0005 s`.
fn plain_seconds(seconds: f64) -> String {
    format!("{seconds} s")
}

#[cfg(test)]
#[path = "duration_words_tests.rs"]
mod tests;
