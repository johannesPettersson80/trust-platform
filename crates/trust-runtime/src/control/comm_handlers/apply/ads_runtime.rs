//! Enable the ADS client in a project's `runtime.toml` after an ADS tag import.
//!
//! This ports `ensureAdsRuntimeEnabled` from the VS Code extension
//! (`editors/vscode/src/networkCanvas/offlineComm.ts:118-165`); design §4.3 B7,
//! specification 25 §5.8. The text edit keeps every other line, comment, and
//! section exactly as it was.

use std::fs;
use std::path::Path;

/// The refusal text of the ported VS Code function, word for word
/// (`editors/vscode/src/networkCanvas/offlineComm.ts:134`).
const MISSING_RUNTIME_TOML: &str = "ADS tag import wrote the selected tags, but runtime.toml is missing so ADS cannot be enabled automatically.";

/// The exact section header that is searched for and written.
const ADS_SECTION: &str = "[runtime.ads]";

/// Enable the ADS client in `<project_root>/runtime.toml` (spec 25 §5.8).
///
/// In `[runtime.ads]` this sets `enabled = true` and `config_path = config_path`
/// (the ported default is `ads.toml`), and adds `worker_tick_interval_ms = 20`
/// only when that key is missing. A missing section is appended at the end, and
/// every other line, comment, and section is kept as it was. An unchanged file is
/// not written. Returns whether the file changed; a missing `runtime.toml` is a
/// refusal carrying the ported message.
pub(crate) fn ensure_ads_runtime_enabled(
    project_root: &Path,
    config_path: &str,
) -> Result<bool, String> {
    let runtime_toml = project_root.join("runtime.toml");
    let before = match fs::read_to_string(&runtime_toml) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(MISSING_RUNTIME_TOML.to_string());
        }
        Err(error) => {
            return Err(format!(
                "failed to read {}: {error}",
                runtime_toml.display()
            ));
        }
    };
    let quoted_path = serde_json::to_string(config_path)
        .map_err(|error| format!("failed to encode ADS config_path: {error}"))?;

    let after = match ads_section_span(&before) {
        Some((start, end)) => {
            let mut section = before[start..end].to_string();
            section = upsert_toml_key(&section, "enabled", "true");
            section = upsert_toml_key(&section, "config_path", quoted_path.as_str());
            if !has_toml_key(&section, "worker_tick_interval_ms") {
                section = upsert_toml_key(&section, "worker_tick_interval_ms", "20");
            }
            format!("{}{}{}", &before[..start], section, &before[end..])
        }
        None => {
            let suffix = if before.ends_with('\n') { "\n" } else { "\n\n" };
            format!(
                "{}{suffix}{ADS_SECTION}\nenabled = true\nconfig_path = {quoted_path}\nworker_tick_interval_ms = 20\n",
                before.trim_end()
            )
        }
    };

    if after == before {
        return Ok(false);
    }
    fs::write(&runtime_toml, &after)
        .map_err(|error| format!("failed to write {}: {error}", runtime_toml.display()))?;
    Ok(true)
}

/// The `(start, end)` byte span of the `[runtime.ads]` section text: from its
/// header line to the next section header, or the end of the file.
fn ads_section_span(text: &str) -> Option<(usize, usize)> {
    let (section_start, search_from) = find_ads_header(text)?;
    let section_end = find_next_section(text, search_from).unwrap_or(text.len());
    Some((section_start, section_end))
}

/// The first line whose content is `[runtime.ads]` followed only by whitespace.
///
/// Returns the header's line start and the end of the ported header match, which
/// may consume the header line's newline and any following blank lines.
fn find_ads_header(text: &str) -> Option<(usize, usize)> {
    for (line_start, content) in line_spans(text) {
        let Some(rest) = content.strip_prefix(ADS_SECTION) else {
            continue;
        };
        if !rest.chars().all(is_js_whitespace) {
            continue;
        }
        let run_start = line_start + ADS_SECTION.len();
        let run_end = text[run_start..]
            .char_indices()
            .find(|(_, ch)| !is_js_whitespace(*ch))
            .map_or(text.len(), |(offset, _)| run_start + offset);
        let match_end = if run_end == text.len() {
            run_end
        } else {
            text[run_start..run_end]
                .char_indices()
                .rfind(|(_, ch)| is_line_terminator(*ch))
                .map_or(run_start, |(offset, _)| run_start + offset)
        };
        return Some((line_start, match_end));
    }
    None
}

/// The start of the next section header at or after `from`, matching
/// `^\s*\[[^\]\r\n]+\]\s*$` in the ported regular expression.
fn find_next_section(text: &str, from: usize) -> Option<usize> {
    line_spans(text)
        .filter(|(line_start, _)| *line_start >= from)
        .find(|(line_start, _)| section_header_at(text, *line_start))
        .map(|(line_start, _)| line_start)
}

/// Whether a section header line begins at `start`.
fn section_header_at(text: &str, start: usize) -> bool {
    let Some(rest) = text[start..]
        .trim_start_matches(is_js_whitespace)
        .strip_prefix('[')
    else {
        return false;
    };
    let Some(close) = rest.find(']') else {
        return false;
    };
    let inside = &rest[..close];
    if inside.is_empty() || inside.contains('\r') || inside.contains('\n') {
        return false;
    }
    let remainder = &rest[close + 1..];
    let line = remainder.split('\n').next().unwrap_or_default();
    let line = line.strip_suffix('\r').unwrap_or(line);
    line.chars().all(is_js_whitespace)
}

/// Replace the line of `key` inside `section` with `key = value`, or append it
/// at the end of the section when it is missing (`upsertTomlKey`).
fn upsert_toml_key(section: &str, key: &str, value: &str) -> String {
    match find_key_line(section, key) {
        Some((start, end)) => {
            let mut updated = String::with_capacity(section.len() + value.len());
            updated.push_str(&section[..start]);
            updated.push_str(key);
            updated.push_str(" = ");
            updated.push_str(value);
            updated.push_str(&section[end..]);
            updated
        }
        None => format!("{}\n{key} = {value}\n", section.trim_end()),
    }
}

/// Whether `section` already carries a `key = ...` line.
fn has_toml_key(section: &str, key: &str) -> bool {
    find_key_line(section, key).is_some()
}

/// The `(start, end)` byte span of the content of the first line that starts
/// with `key` followed by optional whitespace and `=` (`^key\s*=.*$`).
fn find_key_line(text: &str, key: &str) -> Option<(usize, usize)> {
    for (line_start, content) in line_spans(text) {
        let Some(rest) = content.strip_prefix(key) else {
            continue;
        };
        if !rest.trim_start().starts_with('=') {
            continue;
        }
        return Some((line_start, line_start + content.len()));
    }
    None
}

/// Each line's start and content, without its `\n` or `\r\n` terminator.
fn line_spans(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    text.split_inclusive('\n').map(move |line| {
        let start = offset;
        offset += line.len();
        let content = line
            .strip_suffix('\n')
            .map_or(line, |line| line.strip_suffix('\r').unwrap_or(line));
        (start, content)
    })
}

/// JavaScript `\s` for the whitespace the ported patterns admit.
fn is_js_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r' | '\u{000b}' | '\u{000c}')
}

/// A JavaScript line terminator.
fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r')
}

#[cfg(test)]
#[path = "ads_runtime/tests.rs"]
mod tests;
