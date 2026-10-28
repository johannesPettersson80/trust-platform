//! The project's simulation speed (specification 46 §6.1 items 2 and 5; specification 35, B9.2):
//! the speed `simulation.toml` saves in `[simulation] time_scale`, the speeds the chooser offers,
//! saving a chosen speed, and whether the running simulator takes it at once with `config.set`.
//!
//! `simulation.toml` lies in the project folder: the workspace folder when it holds
//! `runtime.toml`, else its `bundle` folder when that holds one, else the workspace folder. A save
//! changes only the `time_scale` line: comments, `enabled`, `[[plants]]`, `[[couplings]]` and every
//! other line stay as they are, and so do the file's line endings.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::scheduler::validate_time_scale;

use super::decode;
use super::project_root::runtime_project_folder;

/// The speeds the chooser offers (specification 46 §6.1 item 2: a whole number from 1 to 100).
pub(super) const SIMULATION_SPEEDS: [u32; 7] = [1, 2, 5, 10, 20, 50, 100];

const SIMULATION_FILE: &str = "simulation.toml";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum Args {
    /// The saved speed and the speeds to offer.
    Read { folder: PathBuf },
    /// Saves `speed`, and names the request that applies it to `target` at once.
    Save {
        folder: PathBuf,
        speed: i64,
        #[serde(default)]
        target: Option<Target>,
    },
}

/// The target the sidebar and the status bar have selected.
#[derive(Deserialize)]
struct Target {
    /// `simulator`, `local` or `remote`.
    kind: String,
    /// `running`, `stopped`, …
    status: String,
    /// Its control endpoint; empty when it has none.
    #[serde(default)]
    endpoint: String,
}

/// `read`: `{ ok, speed, speeds, file }`. `save`: `{ ok, file, live }`, where `live` is the request
/// the extension sends to the running simulator, or `null` when the speed is only saved.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::Read { folder } => {
            let file = simulation_file(&folder);
            let speed = read_file(&file)
                .ok()
                .map_or(1, |text| saved_speed(text.as_deref()));
            Ok(json!({
                "ok": true,
                "speed": speed,
                "speeds": SIMULATION_SPEEDS,
                "file": file.display().to_string(),
            }))
        }
        Args::Save {
            folder,
            speed,
            target,
        } => {
            let speed = validate_time_scale(speed)
                .map_err(|message| format!("The simulation speed {message}."))?;
            let file = simulation_file(&folder);
            let text = read_file(&file)?;
            std::fs::write(&file, with_time_scale(text.as_deref(), speed))
                .map_err(|error| format!("Could not save {}: {error}", file.display()))?;
            let live = target
                .filter(applies_at_once)
                .map(|_| json!({ "request": "config.set", "params": { "simulation.time_scale": speed } }));
            Ok(json!({ "ok": true, "file": file.display().to_string(), "live": live }))
        }
    }
}

/// A speed reaches the runtime at once only when the selected target is a running simulator with
/// a control endpoint (specification 35, A2.4: the selected target decides where a speed goes).
fn applies_at_once(target: &Target) -> bool {
    target.kind == "simulator" && target.status == "running" && !target.endpoint.trim().is_empty()
}

fn simulation_file(folder: &Path) -> PathBuf {
    runtime_project_folder(folder)
        .unwrap_or_else(|| folder.to_path_buf())
        .join(SIMULATION_FILE)
}

/// The file's text, or `None` when it does not exist. A file that exists but cannot be read is an
/// error.
fn read_file(file: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(_) if !file.exists() => Ok(None),
        Err(error) => Err(format!("Could not read {}: {error}", file.display())),
    }
}

/// The speed `[simulation] time_scale` saves: 1 when the file, the table or a whole number from 1
/// to 100 is missing.
fn saved_speed(text: Option<&str>) -> u32 {
    let Some(text) = text else {
        return 1;
    };
    let lines: Vec<&str> = split_lines(text);
    let Some((start, end)) = simulation_table(&lines) else {
        return 1;
    };
    lines[start + 1..end]
        .iter()
        .find_map(|line| time_scale_value(line))
        .and_then(|digits| digits.parse::<u32>().ok())
        .filter(|scale| (1..=100).contains(scale))
        .unwrap_or(1)
}

/// The file text with `[simulation] time_scale = scale`. A missing key is added directly after the
/// table's header; a missing table is inserted before the first table header, after any leading
/// comments; a missing or blank file becomes just the table.
fn with_time_scale(text: Option<&str>, scale: u32) -> String {
    let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
        return format!("[simulation]\ntime_scale = {scale}\n");
    };
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = split_lines(text).into_iter().map(str::to_owned).collect();
    let borrowed: Vec<&str> = lines.iter().map(String::as_str).collect();
    if let Some((start, end)) = simulation_table(&borrowed) {
        let key = (start + 1..end).find(|index| is_time_scale_key(&lines[*index]));
        match key {
            Some(index) => lines[index] = replace_time_scale(&lines[index], scale),
            None => lines.insert(start + 1, format!("time_scale = {scale}")),
        }
        return lines.join(eol);
    }
    match lines.iter().position(|line| is_table_header(line)) {
        None => {
            let mut base = text.to_owned();
            if !base.ends_with('\n') {
                base.push_str(eol);
            }
            format!("{base}{eol}[simulation]{eol}time_scale = {scale}{eol}")
        }
        Some(first_table) => {
            for (offset, line) in [
                "[simulation]".to_owned(),
                format!("time_scale = {scale}"),
                String::new(),
            ]
            .into_iter()
            .enumerate()
            {
                lines.insert(first_table + offset, line);
            }
            lines.join(eol)
        }
    }
}

/// The lines of `text`, split at `\n` or `\r\n`, with a last empty line after a final newline.
fn split_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let last = lines.len() - 1;
    for line in &mut lines[..last] {
        *line = line.strip_suffix('\r').unwrap_or(line);
    }
    lines
}

/// The `[simulation]` table: the index of its header and of the first line after it.
fn simulation_table(lines: &[&str]) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| is_simulation_header(line))?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| is_table_header(line))
        .map_or(lines.len(), |(index, _)| index);
    Some((start, end))
}

/// `[simulation]`, optionally followed by a comment.
fn is_simulation_header(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("[simulation]")
        .is_some_and(is_line_end)
}

/// Any table header, `[name]` or `[[name]]`, optionally followed by a comment.
fn is_table_header(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix('[') else {
        return false;
    };
    let rest = rest.strip_prefix('[').unwrap_or(rest).trim_start();
    let mut chars = rest.char_indices();
    match chars.next() {
        Some((_, first)) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    let name_end = chars
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')))
        .map_or(rest.len(), |(index, _)| index);
    let Some(rest) = rest[name_end..].trim_start().strip_prefix(']') else {
        return false;
    };
    is_line_end(rest.strip_prefix(']').unwrap_or(rest))
}

/// Nothing but blanks, or blanks and a comment.
fn is_line_end(rest: &str) -> bool {
    let rest = rest.trim_start();
    rest.is_empty() || rest.starts_with('#')
}

/// The text after `time_scale` and `=`, when `line` assigns the key.
fn after_time_scale_equals(line: &str) -> Option<&str> {
    line.trim_start()
        .strip_prefix("time_scale")?
        .trim_start()
        .strip_prefix('=')
}

fn is_time_scale_key(line: &str) -> bool {
    after_time_scale_equals(line).is_some()
}

/// The digits of `time_scale = <digits>`, optionally followed by a comment.
fn time_scale_value(line: &str) -> Option<&str> {
    let value = after_time_scale_equals(line)?.trim_start();
    let digits_end = value
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(value.len());
    let digits = &value[..digits_end];
    (!digits.is_empty() && is_line_end(&value[digits_end..])).then_some(digits)
}

/// `line` with the value of its `time_scale` key replaced by `scale`, its indentation, spacing and
/// comment kept.
fn replace_time_scale(line: &str, scale: u32) -> String {
    let Some(after_equals) = after_time_scale_equals(line) else {
        return line.to_owned();
    };
    let value_start = line.len() - after_equals.trim_start().len();
    let rest = &line[value_start..];
    // The value ends at the comment, or at the line's end, without the blanks before either.
    let region_end = rest.find('#').unwrap_or(rest.len());
    let value_end = rest[..region_end].trim_end().len();
    format!("{}{scale}{}", &line[..value_start], &rest[value_end..])
}

#[cfg(test)]
#[path = "simulation_speed_tests.rs"]
mod tests;
