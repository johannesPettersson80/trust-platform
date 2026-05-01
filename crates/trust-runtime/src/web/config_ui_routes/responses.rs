use super::*;

pub(super) fn json_response(
    status: u16,
    body: serde_json::Value,
) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

pub(super) fn structured_error_response(
    status: u16,
    error_code: &str,
    message: &str,
    field_errors: Vec<FieldErrorItem>,
    conflict_version: Option<String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        status,
        json!({
            "ok": false,
            "error_code": error_code,
            "message": message,
            "field_errors": field_errors,
            "conflict_version": conflict_version,
        }),
    )
}
