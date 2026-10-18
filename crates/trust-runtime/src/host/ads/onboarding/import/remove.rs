//! Remove imported ADS points from `ads.toml` and rebuild the generated ST.
//!
//! This is the Rust owner of the VS Code panel's `removeAdsTagFromToml`
//! (`NC/adsTagConfigMutation.ts:13-125`): the same connections match, the same points go, and
//! every other line, blank line and comment is kept. Regeneration reuses
//! [`generate_ads_interface`] and [`parse_ads_toml`] (design §4.3 B7, inventory rows I28 and I40).

use trust_ads_core::SymbolSnapshot;

use crate::ads::{generate_ads_interface, parse_ads_toml};

const CONNECTION_HEADER: &str = "[[connections]]";
const POINT_HEADER: &str = "[[connections.points]]";

/// The ADS device whose imported points are removed: host, target AMS Net ID and AMS port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsTagDevice {
    /// Connection `host` (or legacy `ip`). `None` leaves the host unconstrained.
    pub host: Option<String>,
    /// Connection `target_net_id` (or legacy `ams_net_id`). `None` leaves the Net ID unconstrained.
    pub target_net_id: Option<String>,
    /// Connection AMS port. A connection without `ams_port` counts as 851.
    pub ams_port: u16,
}

/// New `ads.toml` text and the number of points removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdsTagRemoval {
    /// The rewritten `ads.toml` text.
    pub text: String,
    /// How many `[[connections.points]]` blocks were removed.
    pub removed_count: usize,
}

/// Remove the points whose `symbol` (or legacy `path`) is one of `paths` from every connection
/// whose host, target AMS Net ID and AMS port equal `device`.
///
/// A connection left without points is removed with its block. Every other line, blank line and
/// comment is kept byte-for-byte. A connection without `ams_port` counts as 851, and a missing
/// `host` or `target_net_id` on `device` does not constrain that field, as in
/// `removeAdsTagFromToml`.
#[must_use]
pub fn remove_symbol_imports(
    ads_toml_text: &str,
    device: &AdsTagDevice,
    paths: &[String],
) -> AdsTagRemoval {
    let starts = section_starts(ads_toml_text, CONNECTION_HEADER);
    if starts.is_empty() {
        return AdsTagRemoval {
            text: ads_toml_text.to_string(),
            removed_count: 0,
        };
    }
    let mut output = String::with_capacity(ads_toml_text.len());
    output.push_str(&ads_toml_text[..starts[0]]);
    let mut removed_count = 0;
    for (index, start) in starts.iter().copied().enumerate() {
        let end = starts
            .get(index + 1)
            .copied()
            .unwrap_or(ads_toml_text.len());
        let (text, removed) =
            remove_points_from_connection(&ads_toml_text[start..end], device, paths);
        output.push_str(&text);
        removed_count += removed;
    }
    AdsTagRemoval {
        text: output,
        removed_count,
    }
}

/// Rebuild the generated ADS ST from the updated `ads.toml` and every cached snapshot.
///
/// The removal transaction uses this so the generated interface matches the rewritten
/// configuration. It fails when the rewritten configuration or a snapshot is invalid.
pub fn regenerate_ads_interface(
    ads_toml_text: &str,
    snapshots: &[SymbolSnapshot],
) -> Result<String, String> {
    let config =
        parse_ads_toml(ads_toml_text).map_err(|error| format!("invalid ads.toml: {error}"))?;
    let generated =
        generate_ads_interface(&config, snapshots).map_err(|error| error.to_string())?;
    Ok(generated.source)
}

fn remove_points_from_connection(
    block: &str,
    device: &AdsTagDevice,
    paths: &[String],
) -> (String, usize) {
    let starts = section_starts(block, POINT_HEADER);
    if starts.is_empty() {
        return (block.to_string(), 0);
    }
    let header = &block[..starts[0]];
    if !connection_matches(header, device) {
        return (block.to_string(), 0);
    }
    let mut kept = Vec::new();
    let mut removed_count = 0;
    for (index, start) in starts.iter().copied().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(block.len());
        let point = &block[start..end];
        let symbol = toml_string(point, "symbol").or_else(|| toml_string(point, "path"));
        if symbol.is_some_and(|symbol| paths.contains(&symbol)) {
            removed_count += 1;
        } else {
            kept.push(point);
        }
    }
    if removed_count == 0 {
        return (block.to_string(), 0);
    }
    if kept.is_empty() {
        return (String::new(), removed_count);
    }
    let mut text = String::with_capacity(header.len() + block.len());
    text.push_str(header);
    for point in kept {
        text.push_str(point);
    }
    (text, removed_count)
}

fn connection_matches(header: &str, device: &AdsTagDevice) -> bool {
    let port = toml_integer(header, "ams_port").unwrap_or(851);
    if port != u64::from(device.ams_port) {
        return false;
    }
    let net_id = non_empty(toml_string(header, "target_net_id"))
        .or_else(|| non_empty(toml_string(header, "ams_net_id")));
    let host =
        non_empty(toml_string(header, "host")).or_else(|| non_empty(toml_string(header, "ip")));
    let target_net_id = non_empty(device.target_net_id.clone());
    let target_host = non_empty(device.host.clone());
    if let (Some(target), Some(found)) = (target_net_id.as_deref(), net_id.as_deref())
        && target != found
    {
        return false;
    }
    if let (Some(target), Some(found)) = (target_host.as_deref(), host.as_deref())
        && target != found
    {
        return false;
    }
    (target_net_id.is_some() && net_id.is_some())
        || (target_host.is_some() && host.is_some())
        || (target_net_id.is_none() && target_host.is_none())
}

/// The byte offset where a `marker` section header begins for the TypeScript multi-line regex
/// `^\s*<marker>\s*(?:#.*)?$`: the start of the first line in the whitespace run before it.
fn section_starts(text: &str, marker: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut search = 0;
    while let Some(offset) = text[search..].find(marker) {
        let marker_start = search + offset;
        let marker_end = marker_start + marker.len();
        let line_start = text[..marker_start]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_end = text[marker_end..]
            .find('\n')
            .map_or(text.len(), |index| marker_end + index);
        if text[line_start..marker_start]
            .chars()
            .all(char::is_whitespace)
            && blank_or_comment(&text[marker_end..line_end])
        {
            starts.push(section_start(text, marker_start));
        }
        search = marker_end.max(search + 1);
    }
    starts
}

fn section_start(text: &str, marker_start: usize) -> usize {
    let run_start = text[..marker_start]
        .rfind(|ch: char| !ch.is_whitespace())
        .map_or(0, |index| {
            index + text[index..].chars().next().map_or(1, char::len_utf8)
        });
    match text[run_start..marker_start].find('\n') {
        Some(offset) => run_start + offset + 1,
        None => run_start,
    }
}

fn blank_or_comment(rest: &str) -> bool {
    let trimmed = rest.trim_start_matches(char::is_whitespace);
    trimmed.is_empty() || trimmed.starts_with('#')
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn toml_assignment<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    for line in block.lines() {
        let Some(rest) = line.trim_start().strip_prefix(key) else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        return Some(rest);
    }
    None
}

fn toml_string(block: &str, key: &str) -> Option<String> {
    let raw = toml_assignment(block, key).map(str::trim_start)?;
    if let Some(rest) = raw.strip_prefix('"') {
        let end = double_quoted_end(rest)?;
        let literal = format!("\"{}\"", &rest[..end]);
        let value = serde_json::from_str::<String>(&literal).ok()?;
        return blank_or_comment(&rest[end + 1..]).then_some(value);
    }
    if let Some(rest) = raw.strip_prefix('\'') {
        let end = rest.find('\'')?;
        return blank_or_comment(&rest[end + 1..]).then_some(rest[..end].to_string());
    }
    None
}

/// The index of the closing double quote in `text`, whose first character follows the opening
/// quote. Backslash escapes are honoured, as TOML basic strings and `JSON.parse` do.
fn double_quoted_end(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(index),
            _ => {}
        }
    }
    None
}

fn toml_integer(block: &str, key: &str) -> Option<u64> {
    let raw = toml_assignment(block, key).map(str::trim_start)?;
    let digits = raw
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() || !blank_or_comment(&raw[digits.len()..]) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod remove_tests;
