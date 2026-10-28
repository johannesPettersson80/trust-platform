//! The words that name a kept recording to the engineer (specification 35, B9.9 and B10.3): what the run recorded and when its
//! first sample was captured, in place of the study's `rec-…` identifier, which names the file and says nothing to a reader.

use super::RecordingFile;
use crate::experiment::plan::ControlExperimentKind;

/// Milliseconds in a day.
const DAY_MS: u64 = 86_400_000;

/// The words that name `file`, for example "PRBS recorded 2026-10-22 14:05:31 UTC": the experiment the run wrote first and the
/// runtime's wall-clock time of the first sample in UTC (annex E4.6: `captured_at_ms`, for display only). A run that wrote no
/// experiment is "Recording of …", or "Opened recording of …" when it was opened from a file. A recording without samples names no
/// time.
#[must_use]
pub fn recording_label(file: &RecordingFile) -> String {
    let experiment = file
        .context
        .as_ref()
        .and_then(|context| context.experiments.first())
        .map(|experiment| experiment_words(experiment.kind));
    let time = file
        .run
        .samples
        .first()
        .map(|sample| utc_text(sample.captured_at_ms));
    let plain = if file.opened {
        "Opened recording"
    } else {
        "Recording"
    };
    match (experiment, time) {
        (Some(words), Some(time)) => format!("{words} recorded {time}"),
        (Some(words), None) => words.to_owned(),
        (None, Some(time)) => format!("{plain} of {time}"),
        (None, None) => plain.to_owned(),
    }
}

/// The words of an experiment's kind, as the Method list names the methods that run it (B3.3).
fn experiment_words(kind: ControlExperimentKind) -> &'static str {
    match kind {
        ControlExperimentKind::Observation => "Observation",
        ControlExperimentKind::SetpointStep => "Setpoint step",
        ControlExperimentKind::LoadStep => "Load step",
        ControlExperimentKind::MvStep | ControlExperimentKind::OutputStep => "Step response",
        ControlExperimentKind::MvPulse => "Pulse response",
        ControlExperimentKind::Relay => "Feedback relay",
        ControlExperimentKind::Prbs => "PRBS",
        ControlExperimentKind::Multisine => "Multisine",
        ControlExperimentKind::ProjectSequence => "Project experiment",
    }
}

/// `ms` milliseconds after the Unix epoch as `YYYY-MM-DD HH:MM:SS UTC` (the proleptic Gregorian calendar, the days counted back to a
/// civil date as in H. Hinnant, "chrono-Compatible Low-Level Date Algorithms").
fn utc_text(ms: u64) -> String {
    let days = ms / DAY_MS;
    let seconds_of_day = (ms % DAY_MS) / 1000;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        seconds_of_day / 3600,
        seconds_of_day % 3600 / 60,
        seconds_of_day % 60
    )
}

/// The civil date of `days` days after 1970-01-01.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[path = "label_tests.rs"]
mod tests;
