//! The export of a chart image (specification 35, B10.5): the PNG the page renders, or the text the page sends, with the file name,
//! the save filter and the bytes the extension writes. The engine checks the page's message against the session the panel shows
//! (the project, the editor session, the loop or plant revision), the MIME type of its data, the extension of its file name and the
//! size, and decodes the data URL. The extension shows the save dialog and writes the bytes.

#![allow(clippy::module_name_repetitions)] // The types name their concept: the chart export.

use base64::Engine as _;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};

/// The kind of the page's message.
const KIND: &str = "control-studio.chart.export";
/// The MIME types a chart export may carry.
const ALLOWED_MIME_TYPES: [&str; 3] = ["image/png", "text/csv", "application/json"];
/// The largest data URL the page may send, in bytes.
const MAX_DATA_BYTES: usize = 64 * 1024 * 1024;
/// The largest export file (B10.5).
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

const STALE_SESSION: &str = "The chart export session is stale or invalid.";
const UNSUPPORTED_DATA: &str = "Unsupported chart export data.";
const INVALID_DATA: &str = "Chart export data is not valid.";
const TOO_LARGE_DATA: &str = "The chart export is too large.";
const MIME_MISMATCH: &str = "Export MIME type does not match its data.";
const BAD_FILE_NAME: &str = "Choose a filename with the correct export extension.";
const TOO_LARGE_FILE: &str = "The chart export is larger than 16 MiB.";

/// The arguments of `controlStudio/chartExport`: the page's message as it arrived, and the session the panel shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartExportRequest {
    /// The page's `control-studio.chart.export` message; the engine checks its envelope and its payload.
    pub message: ChartExportMessage,
    /// The session the panel shows: the project, the editor session, and the loop or the plant it shows.
    pub session: ChartExportSession,
}

/// The envelope of the page's chart export message. Its fields are optional, so the engine, not the decoder, names what is missing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChartExportMessage {
    /// The envelope's schema version; 1.
    pub schema_version: Option<u64>,
    /// The message kind; `control-studio.chart.export`.
    pub kind: Option<String>,
    /// The project the message belongs to.
    pub project_id: Option<String>,
    /// The editor session the message belongs to.
    pub editor_session_id: Option<String>,
    /// The loop the message belongs to, for a loop's chart.
    pub loop_id: Option<String>,
    /// The loop revision the message expects.
    pub expected_loop_revision: Option<String>,
    /// The plant the message belongs to, for a plant's chart.
    pub plant_id: Option<String>,
    /// The plant revision the message expects.
    pub expected_plant_revision: Option<String>,
    /// The export itself.
    pub payload: Option<ChartExportPayload>,
}

/// The export the page asks for: its request identifier, the data URL it carries, the data URL's MIME type, and the file name the
/// page suggests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChartExportPayload {
    /// The page's identifier of the request, which the reply carries back.
    pub request_id: Option<String>,
    /// The export's data, as a data URL.
    pub data: Option<String>,
    /// The MIME type the page states for the data.
    pub mime_type: Option<String>,
    /// The file name the page suggests, when it suggests one.
    pub filename: Option<String>,
}

/// The session the panel shows: the project and editor session, and the loop or the plant with its revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartExportSession {
    /// The project the panel shows.
    pub project_id: String,
    /// The editor session of the panel.
    pub editor_session_id: String,
    /// The loop the panel shows, when it shows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_id: Option<String>,
    /// The loop revision the panel shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_revision: Option<String>,
    /// The plant the panel shows, when it shows a plant and no loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant_id: Option<String>,
    /// The plant revision the panel shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plant_revision: Option<String>,
}

/// What `controlStudio/chartExport` answers (B10.5): the suggested file name, the save filter's label and extension, and the file's
/// bytes as base64. The extension shows the save dialog with them and writes the bytes.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChartExportAnswer {
    /// The file name the export is suggested under.
    pub file_name: String,
    /// The label of the save dialog's filter, such as `PNG image`.
    pub filter_label: String,
    /// The extension the save dialog's filter allows, without its dot.
    pub extension: String,
    /// The file's bytes, in standard base64.
    pub bytes_base64: String,
}

/// Answers `controlStudio/chartExport` (B10.5): the export of the page's message, when the message is an export of the session the
/// panel shows, its data is a data URL of an allowed MIME type that matches the message, its file name is a plain name with the
/// type's extension, and the file is at most 16 MiB.
///
/// # Errors
/// The message is not an export of the session; its data is too large, not a data URL of an allowed type, or not valid; its MIME
/// type does not match its data; its file name is not a plain name with the type's extension; or its file is larger than 16 MiB.
pub fn export(request: &ChartExportRequest) -> Result<ChartExportAnswer, Failure> {
    let Some((payload, data, mime_type)) = session_payload(&request.message, &request.session)
    else {
        return Err(refuse(STALE_SESSION));
    };
    let url = read_data_url(data).map_err(refuse)?;
    if url.mime_type != mime_type {
        return Err(refuse(MIME_MISMATCH));
    }
    let file_name = export_name(payload.filename.as_deref(), &url.mime_type)
        .ok_or_else(|| refuse(BAD_FILE_NAME))?;
    if url.bytes.len() > MAX_FILE_BYTES {
        return Err(refuse(TOO_LARGE_FILE));
    }
    Ok(ChartExportAnswer {
        file_name,
        filter_label: filter_label(&url.mime_type).to_string(),
        extension: extension(&url.mime_type).to_string(),
        bytes_base64: base64::engine::general_purpose::STANDARD.encode(&url.bytes),
    })
}

fn refuse(text: &str) -> Failure {
    Failure::new(text, Cause::Unclassified, NextAction::None)
}

/// The payload, with its data and MIME type, when the message is an export of the session.
fn session_payload<'a>(
    message: &'a ChartExportMessage,
    session: &ChartExportSession,
) -> Option<(&'a ChartExportPayload, &'a str, &'a str)> {
    if message.schema_version != Some(1)
        || message.kind.as_deref() != Some(KIND)
        || !same_session(message, session)
    {
        return None;
    }
    let payload = message.payload.as_ref()?;
    if !has_text(payload.request_id.as_deref()) {
        return None;
    }
    Some((
        payload,
        payload.data.as_deref()?,
        payload.mime_type.as_deref()?,
    ))
}

/// Whether the message names the session's project, editor session and revision: a loop's when the session shows a loop, else a
/// plant's.
fn same_session(message: &ChartExportMessage, session: &ChartExportSession) -> bool {
    if message.project_id.as_deref() != Some(session.project_id.as_str())
        || message.editor_session_id.as_deref() != Some(session.editor_session_id.as_str())
    {
        return false;
    }
    if has_text(session.loop_id.as_deref()) && has_text(session.loop_revision.as_deref()) {
        return message.loop_id == session.loop_id
            && message.expected_loop_revision == session.loop_revision;
    }
    has_text(session.plant_id.as_deref())
        && has_text(session.plant_revision.as_deref())
        && message.plant_id == session.plant_id
        && message.expected_plant_revision == session.plant_revision
}

fn has_text(text: Option<&str>) -> bool {
    text.is_some_and(|text| !text.is_empty())
}

/// A data URL's MIME type and the bytes it carries.
struct DataUrl {
    mime_type: String,
    bytes: Vec<u8>,
}

/// Reads a data URL as the page writes it (RFC 2397): `data:<type>[;parameters],<body>`, whose body is base64 when a parameter says
/// so and percent-encoded text otherwise.
fn read_data_url(data: &str) -> Result<DataUrl, &'static str> {
    if data.len() > MAX_DATA_BYTES {
        return Err(TOO_LARGE_DATA);
    }
    let (header, body) = data
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(','))
        .ok_or(UNSUPPORTED_DATA)?;
    let (mime_type, parameters) = match header.split_once(';') {
        Some((mime_type, parameters)) if !parameters.is_empty() => (mime_type, Some(parameters)),
        Some(_) => return Err(UNSUPPORTED_DATA),
        None => (header, None),
    };
    if !ALLOWED_MIME_TYPES.contains(&mime_type) {
        return Err(UNSUPPORTED_DATA);
    }
    let is_base64 = parameters
        .is_some_and(|parameters| parameters.split(';').any(|parameter| parameter == "base64"));
    let bytes = decode_body(body, is_base64).ok_or(INVALID_DATA)?;
    Ok(DataUrl {
        mime_type: mime_type.to_string(),
        bytes,
    })
}

fn decode_body(body: &str, is_base64: bool) -> Option<Vec<u8>> {
    if is_base64 {
        decode_base64(body)
    } else {
        percent_decode(body)
    }
}

/// Base64 as `atob` reads it: ASCII whitespace is ignored, and the padding is optional where the length allows it.
fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut compact: String = text
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    if compact.len().is_multiple_of(4) {
        if compact.ends_with("==") {
            compact.truncate(compact.len() - 2);
        } else if compact.ends_with('=') {
            compact.pop();
        }
    }
    if compact.len() % 4 == 1 {
        return None;
    }
    while !compact.len().is_multiple_of(4) {
        compact.push('=');
    }
    base64::engine::general_purpose::STANDARD
        .decode(compact)
        .ok()
}

/// Percent-decoding as `decodeURIComponent` reads it: each `%XX` is one byte, the bytes must be UTF-8, and every other character
/// stands for its own UTF-8 bytes.
fn percent_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let [high, low]: [u8; 2] = bytes.get(index + 1..index + 3)?.try_into().ok()?;
            decoded.push(hex_value(high)? * 16 + hex_value(low)?);
            index += 3;
        } else {
            decoded.push(byte);
            index += 1;
        }
    }
    std::str::from_utf8(&decoded).ok()?;
    Some(decoded)
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

/// The file name of the export: the page's name, trimmed, or `control-studio-export` when the page gives none. A name holds no
/// path, and it ends with the type's extension in any case.
fn export_name(filename: Option<&str>, mime_type: &str) -> Option<String> {
    let extension = extension(mime_type);
    let name = match filename.map(str::trim) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => format!("control-studio-export.{extension}"),
    };
    let plain = !name.contains('/') && !name.contains('\\');
    (plain && name.to_lowercase().ends_with(&format!(".{extension}"))).then_some(name)
}

/// The extension of a MIME type, without its dot.
fn extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "text/csv" => "csv",
        _ => "json",
    }
}

/// The label of the save dialog's filter for a MIME type.
fn filter_label(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "PNG image",
        "text/csv" => "CSV data",
        _ => "JSON data",
    }
}

#[cfg(test)]
#[path = "chart_export_tests.rs"]
mod tests;
