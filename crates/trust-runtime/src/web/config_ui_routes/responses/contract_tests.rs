use super::*;

fn body(response: Response<std::io::Cursor<Vec<u8>>>) -> serde_json::Value {
    serde_json::from_slice(response.into_reader().get_ref()).expect("JSON body")
}

fn has_json_content_type(response: &Response<std::io::Cursor<Vec<u8>>>) -> bool {
    response.headers().iter().any(|header| {
        header.field.equiv("Content-Type")
            && header
                .value
                .as_str()
                .eq_ignore_ascii_case("application/json")
    })
}

#[test]
fn json_response_preserves_requested_status_and_exact_body() {
    let response = json_response(202, json!({"ok": true, "value": [1, 2, 3]}));
    assert_eq!(response.status_code(), StatusCode(202));
    assert_eq!(body(response), json!({"ok": true, "value": [1, 2, 3]}));
}

#[test]
fn json_response_declares_json_content_type() {
    assert!(has_json_content_type(&json_response(
        200,
        json!({"ok": true})
    )));
}

#[test]
fn structured_error_preserves_stable_fields_and_ordered_field_errors() {
    let response = structured_error_response(
        422,
        "invalid_config",
        "configuration is invalid",
        vec![
            FieldErrorItem {
                path: "runtime.web.listen".to_string(),
                hint: "must be loopback".to_string(),
            },
            FieldErrorItem {
                path: "runtime.control.endpoint".to_string(),
                hint: "is required".to_string(),
            },
        ],
        None,
    );
    assert_eq!(response.status_code(), StatusCode(422));
    assert_eq!(
        body(response),
        json!({
            "ok": false,
            "error_code": "invalid_config",
            "message": "configuration is invalid",
            "field_errors": [
                {
                    "path": "runtime.web.listen",
                    "hint": "must be loopback",
                },
                {
                    "path": "runtime.control.endpoint",
                    "hint": "is required",
                }
            ],
            "conflict_version": null,
        })
    );
}

#[test]
fn structured_conflict_preserves_current_version() {
    let response = structured_error_response(
        409,
        "conflict",
        "stale revision",
        Vec::new(),
        Some("sha256-current".to_string()),
    );
    assert_eq!(response.status_code(), StatusCode(409));
    assert_eq!(body(response)["conflict_version"], "sha256-current");
}

#[test]
fn structured_error_is_json_and_never_reports_success() {
    let response =
        structured_error_response(403, "forbidden", "role is insufficient", Vec::new(), None);
    assert!(has_json_content_type(&response));
    assert_eq!(body(response)["ok"], false);
}
