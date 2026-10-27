//! The small checks of new pages, the PLC cycle, languages and settings (row H13).
//!
//! - `newPage { name, pages, hasScreen }` → `{ file, screen? }`: where a page made from the
//!   program goes (Create Pages from Program, Create Settings Page; specification 39, §9 item 3):
//!   `hmi/pages/<name>.toml`, else the first free of `<name>-2`, `<name>-3`, …, never over a page
//!   of `pages`; `screen` is the screen layout a project without one gets (§4 item 1).
//! - `cycle { text }` → `{ milliseconds }` or `{ problem }`: the control cycle Create PLC code
//!   asks for in a project without a task (§10.3 item 3), a whole number of milliseconds from 1 to
//!   60 000.
//! - `languages { languages }` → `{ problem? }`: a project has at least one language (§14.5
//!   item 2).
//! - `sameSetting { project, running }` → `{ same }`: whether a setting's value in the project and
//!   the value the PLC has are the same, for Keep in project (§21.4 item 6): numbers when they
//!   differ by at most 10⁻⁹ of the larger magnitude, or of 1 when both are smaller, so a rounding
//!   difference is no change; any other values when they are equal.
//!
//! A check answers `ok: true`; `problem` is the text the input box or the dialog shows.

use serde::Deserialize;
use serde_json::{json, Value};

use super::decode;

/// The shortest and longest control cycle, in milliseconds.
const CYCLE_RANGE: std::ops::RangeInclusive<u32> = 1..=60_000;

/// What an unusable control cycle is told.
const CYCLE_PROBLEM: &str = "Enter a whole number from 1 to 60000 milliseconds.";

/// What an empty language list is told.
const LANGUAGES_PROBLEM: &str = "A project has at least one language.";

/// The largest difference between two numbers that are the same setting, relative to the larger
/// magnitude, or to 1.
const SAME_SETTING_TOLERANCE: f64 = 1e-9;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    NewPage {
        name: String,
        #[serde(default)]
        pages: Vec<String>,
        #[serde(rename = "hasScreen")]
        has_screen: bool,
    },
    Cycle {
        text: String,
    },
    Languages {
        languages: Vec<String>,
    },
    SameSetting {
        #[serde(default)]
        project: Value,
        running: Value,
    },
}

/// The answer of `trust-lsp.hmiChecks`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::NewPage {
            name,
            pages,
            has_screen,
        } => new_page(&name, &pages, has_screen),
        Args::Cycle { text } => Ok(match cycle(&text) {
            Some(milliseconds) => json!({ "ok": true, "milliseconds": milliseconds }),
            None => json!({ "ok": true, "problem": CYCLE_PROBLEM }),
        }),
        Args::Languages { languages } => Ok(if languages.is_empty() {
            json!({ "ok": true, "problem": LANGUAGES_PROBLEM })
        } else {
            json!({ "ok": true })
        }),
        Args::SameSetting { project, running } => {
            Ok(json!({ "ok": true, "same": same_setting(&project, &running) }))
        }
    }
}

fn new_page(name: &str, pages: &[String], has_screen: bool) -> Result<Value, String> {
    if !super::files::is_file_name(name) {
        return Err(format!("\"{name}\" cannot name a page."));
    }
    let free =
        super::pages::free_name(name, |candidate| pages.iter().any(|page| page == candidate));
    let mut answer = json!({ "ok": true, "file": format!("hmi/pages/{free}.toml") });
    if !has_screen {
        answer["screen"] = json!(super::screens::default_screen());
    }
    Ok(answer)
}

/// The control cycle `text` gives, in milliseconds: ASCII digits only, from 1 to 60 000.
fn cycle(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>()
        .ok()
        .filter(|milliseconds| CYCLE_RANGE.contains(milliseconds))
}

/// Whether the project's value of a setting and the PLC's are the same.
fn same_setting(project: &Value, running: &Value) -> bool {
    match (project.as_f64(), running.as_f64()) {
        (Some(project), Some(running)) => {
            let scale = 1_f64.max(project.abs()).max(running.abs());
            (project - running).abs() <= SAME_SETTING_TOLERANCE * scale
        }
        _ => project == running,
    }
}

#[cfg(test)]
#[path = "checks_tests.rs"]
mod tests;
