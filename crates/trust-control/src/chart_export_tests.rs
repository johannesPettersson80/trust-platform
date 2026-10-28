//! Specification 35, B10.5: the engine's answer to the page's chart export. The tests pin the session check, the data URL forms the
//! page writes, the MIME and file name rules, and the size limits.

use serde_json::json;

use super::*;

const DATA_PNG: &str = "data:image/png;base64,aGk=";

fn session() -> ChartExportSession {
    ChartExportSession {
        project_id: "project".to_string(),
        editor_session_id: "editor".to_string(),
        loop_id: Some("loop".to_string()),
        loop_revision: Some("rev".to_string()),
        plant_id: None,
        plant_revision: None,
    }
}

/// The page's message for a loop's session, with `data` of `mime_type` and the file name `filename`.
fn message(data: &str, mime_type: &str, filename: Option<&str>) -> ChartExportMessage {
    ChartExportMessage {
        schema_version: Some(1),
        kind: Some(KIND.to_string()),
        project_id: Some("project".to_string()),
        editor_session_id: Some("editor".to_string()),
        loop_id: Some("loop".to_string()),
        expected_loop_revision: Some("rev".to_string()),
        plant_id: None,
        expected_plant_revision: None,
        payload: Some(ChartExportPayload {
            request_id: Some("export-1".to_string()),
            data: Some(data.to_string()),
            mime_type: Some(mime_type.to_string()),
            filename: filename.map(str::to_string),
        }),
    }
}

/// The message for the loop's session, changed by `change`.
fn changed(change: impl FnOnce(&mut ChartExportMessage)) -> ChartExportMessage {
    let mut message = message(DATA_PNG, "image/png", None);
    change(&mut message);
    message
}

fn export_of(message: ChartExportMessage) -> Result<ChartExportAnswer, Failure> {
    export(&ChartExportRequest {
        message,
        session: session(),
    })
}

fn bytes_of(answer: &ChartExportAnswer) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(&answer.bytes_base64)
        .expect("the answer's bytes are base64")
}

#[test]
fn a_png_of_the_session_is_answered_with_its_name_filter_and_bytes() {
    let answer = export_of(message(DATA_PNG, "image/png", Some("response.png")))
        .expect("the export of the session is answered");
    assert_eq!(answer.file_name, "response.png");
    assert_eq!(answer.filter_label, "PNG image");
    assert_eq!(answer.extension, "png");
    assert_eq!(bytes_of(&answer), b"hi");
}

#[test]
fn the_answer_names_its_fields_as_the_extension_reads_them() {
    let answer = export_of(message(DATA_PNG, "image/png", None)).expect("the export");
    let json = serde_json::to_value(&answer).expect("the answer serializes");
    assert_eq!(
        json,
        json!({
            "fileName": "control-studio-export.png",
            "filterLabel": "PNG image",
            "extension": "png",
            "bytesBase64": "aGk="
        })
    );
}

#[test]
fn the_page_message_decodes_with_the_envelope_fields_it_carries() {
    let request: ChartExportRequest = serde_json::from_value(json!({
        "message": {
            "schemaVersion": 1, "kind": "control-studio.chart.export", "messageId": "m1",
            "projectId": "project", "editorSessionId": "editor", "loopId": "loop", "expectedLoopRevision": "rev",
            "payload": { "requestId": "export-1", "data": DATA_PNG, "mimeType": "image/png", "filename": "r.png" }
        },
        "session": { "projectId": "project", "editorSessionId": "editor", "loopId": "loop", "loopRevision": "rev" }
    }))
    .expect("the page's message decodes");
    let answer = export(&request).expect("the export is answered");
    assert_eq!(answer.file_name, "r.png");
}

#[test]
fn a_plant_chart_is_checked_against_the_plant_and_its_revision() {
    let plant_session = ChartExportSession {
        loop_id: None,
        loop_revision: None,
        plant_id: Some("heater".to_string()),
        plant_revision: Some("plant-rev".to_string()),
        ..session()
    };
    let mut plant_message = message("data:text/csv,a", "text/csv", None);
    plant_message.loop_id = None;
    plant_message.expected_loop_revision = None;
    plant_message.plant_id = Some("heater".to_string());
    plant_message.expected_plant_revision = Some("plant-rev".to_string());
    let answer = export(&ChartExportRequest {
        message: plant_message.clone(),
        session: plant_session.clone(),
    })
    .expect("a plant chart of its plant is answered");
    assert_eq!(answer.file_name, "control-studio-export.csv");

    plant_message.expected_plant_revision = Some("old".to_string());
    let stale = export(&ChartExportRequest {
        message: plant_message,
        session: plant_session,
    })
    .expect_err("a plant chart of a stale revision is refused");
    assert_eq!(stale.message, STALE_SESSION);
}

#[test]
fn a_message_of_another_session_revision_or_envelope_is_stale() {
    let others = [
        changed(|message| message.expected_loop_revision = Some("old".to_string())),
        changed(|message| message.editor_session_id = Some("other".to_string())),
        changed(|message| message.project_id = Some("other".to_string())),
        changed(|message| message.schema_version = Some(2)),
        changed(|message| message.kind = Some("control-studio.other".to_string())),
        changed(|message| message.loop_id = None),
        changed(|message| message.payload = None),
        changed(|message| {
            if let Some(payload) = message.payload.as_mut() {
                payload.request_id = Some(String::new());
            }
        }),
        changed(|message| {
            if let Some(payload) = message.payload.as_mut() {
                payload.data = None;
            }
        }),
    ];
    for other in others {
        let error = export_of(other).expect_err("a message of another session is refused");
        assert_eq!(error.message, STALE_SESSION);
    }
}

#[test]
fn a_percent_encoded_text_export_carries_its_utf8_bytes() {
    let answer = export_of(message(
        "data:text/csv;charset=utf-8,a%2Cb%0A%C3%A9",
        "text/csv",
        Some("samples.csv"),
    ))
    .expect("the text export is answered");
    assert_eq!(bytes_of(&answer), "a,b\n\u{e9}".as_bytes());
    assert_eq!(answer.filter_label, "CSV data");
    assert_eq!(answer.extension, "csv");
}

#[test]
fn base64_without_padding_or_with_spaces_is_read_as_atob_reads_it() {
    let answer = export_of(message("data:image/png;base64,aG k", "image/png", None))
        .expect("the export is answered");
    assert_eq!(bytes_of(&answer), b"hi");
}

#[test]
fn data_that_is_not_a_data_url_of_an_allowed_type_is_unsupported() {
    for data in [
        "image/png;base64,aGk=",
        "data:text/html;base64,aGk=",
        "data:;base64,aGk=",
        "data:image/png;,aGk=",
    ] {
        let error =
            export_of(message(data, "image/png", None)).expect_err("unsupported data is refused");
        assert_eq!(error.message, UNSUPPORTED_DATA, "{data}");
    }
}

#[test]
fn data_that_does_not_decode_is_not_valid() {
    for (data, mime_type) in [
        ("data:image/png;base64,@@@@", "image/png"),
        ("data:text/csv,%G1", "text/csv"),
        ("data:text/csv,%", "text/csv"),
        ("data:text/csv,%C3", "text/csv"),
    ] {
        let error = export_of(message(data, mime_type, None)).expect_err("invalid data is refused");
        assert_eq!(error.message, INVALID_DATA, "{data}");
    }
}

#[test]
fn a_mime_type_that_does_not_match_its_data_is_refused() {
    let error = export_of(message(DATA_PNG, "text/csv", None))
        .expect_err("a mismatched MIME type is refused");
    assert_eq!(error.message, MIME_MISMATCH);
}

#[test]
fn a_file_name_must_be_plain_and_carry_the_type_extension() {
    for name in [
        "response.txt",
        "../response.png",
        "folder\\response.png",
        "response",
    ] {
        let error = export_of(message(DATA_PNG, "image/png", Some(name)))
            .expect_err("a name the save would not take is refused");
        assert_eq!(error.message, BAD_FILE_NAME, "{name}");
    }
}

#[test]
fn a_missing_or_blank_name_takes_the_fallback_with_the_type_extension() {
    let missing = export_of(message(DATA_PNG, "image/png", None)).expect("the export");
    assert_eq!(missing.file_name, "control-studio-export.png");
    let blank = export_of(message(DATA_PNG, "image/png", Some("   "))).expect("the export");
    assert_eq!(blank.file_name, "control-studio-export.png");
}

#[test]
fn the_extension_matches_in_any_case_and_the_name_is_trimmed() {
    let answer = export_of(message(DATA_PNG, "image/png", Some("  RESPONSE.PNG  ")))
        .expect("the export is answered");
    assert_eq!(answer.file_name, "RESPONSE.PNG");
}

#[test]
fn a_file_of_exactly_16_mib_is_accepted_and_one_byte_more_is_refused() {
    let largest = vec![0_u8; MAX_FILE_BYTES];
    let data = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&largest)
    );
    let answer = export_of(message(&data, "image/png", None)).expect("a 16 MiB file is accepted");
    assert_eq!(bytes_of(&answer).len(), MAX_FILE_BYTES);

    let larger = vec![0_u8; MAX_FILE_BYTES + 1];
    let data = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&larger)
    );
    let error = export_of(message(&data, "image/png", None)).expect_err("a larger file is refused");
    assert_eq!(error.message, TOO_LARGE_FILE);
}

#[test]
fn a_data_url_larger_than_64_mib_is_too_large() {
    let data = format!("data:image/png;base64,{}", "A".repeat(MAX_DATA_BYTES));
    let error =
        export_of(message(&data, "image/png", None)).expect_err("too large data is refused");
    assert_eq!(error.message, TOO_LARGE_DATA);
}
