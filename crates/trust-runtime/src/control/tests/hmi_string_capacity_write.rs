// HMI writes to STRING points count the capacity in UTF-8 bytes, and a program variable
// declared STRING without a length has the capacity 80 (docs/IEC_DECISIONS.md 2026-10-17,
// specification 10 §10.2).

#[test]
fn hmi_write_string_capacity_counts_utf8_bytes() {
    // STRING[3] holds three bytes of UTF-8. "ÄB" is three bytes and fits; "ÄÖ" is two
    // characters in four bytes and is refused.
    let source = r#"
PROGRAM Main
VAR
    label : STRING[3] := 'OK';
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-write-string-bytes");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = ["resource/RESOURCE/program/Main/field/label"]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);

    let fits = handle_request_value(
        json!({
            "id": 80,
            "type": "hmi.write",
            "params": {
                "id": "resource/RESOURCE/program/Main/field/label",
                "value": "ÄB"
            }
        }),
        &state,
        None,
    );
    assert!(fits.ok, "three bytes fit STRING[3]: {:?}", fits.error);
    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].value, Value::String("ÄB".into()));

    let too_long = handle_request_value(
        json!({
            "id": 81,
            "type": "hmi.write",
            "params": {
                "id": "resource/RESOURCE/program/Main/field/label",
                "value": "ÄÖ"
            }
        }),
        &state,
        None,
    );
    assert!(!too_long.ok, "four bytes do not fit STRING[3]");
    assert_eq!(
        too_long.error_code.as_deref(),
        Some("runtime_string_capacity_exceeded")
    );
    assert!(state.debug.drain_var_writes().is_empty());

    fs::remove_dir_all(root).ok();
}

#[test]
fn hmi_write_to_a_string_without_a_length_refuses_more_than_80_bytes() {
    // STRING and STRING[80] are one type, so both points admit 80 bytes and refuse 81.
    let source = r#"
PROGRAM Main
VAR
    plain : STRING := 'OK';
    sized : STRING[80] := 'OK';
END_VAR
END_PROGRAM
"#;
    let root = temp_dir("hmi-write-string-default");
    write_file(
        &root.join("hmi.toml"),
        r#"
[write]
enabled = true
allow = [
    "resource/RESOURCE/program/Main/field/plain",
    "resource/RESOURCE/program/Main/field/sized",
]
"#,
    );

    let mut state = hmi_test_state(source);
    set_hmi_project_root(&mut state, &root);

    for field in ["plain", "sized"] {
        let target = format!("resource/RESOURCE/program/Main/field/{field}");
        let longest = "x".repeat(80);
        let fits = handle_request_value(
            json!({
                "id": 82,
                "type": "hmi.write",
                "params": { "id": target, "value": longest }
            }),
            &state,
            None,
        );
        assert!(fits.ok, "80 bytes fit {field}: {:?}", fits.error);
        let writes = state.debug.drain_var_writes();
        assert_eq!(writes.len(), 1, "{field}");
        assert_eq!(writes[0].value, Value::String(longest.as_str().into()));

        let too_long = handle_request_value(
            json!({
                "id": 83,
                "type": "hmi.write",
                "params": { "id": target, "value": "x".repeat(81) }
            }),
            &state,
            None,
        );
        assert!(!too_long.ok, "81 bytes do not fit {field}");
        assert_eq!(
            too_long.error_code.as_deref(),
            Some("runtime_string_capacity_exceeded"),
            "{field}"
        );
        assert!(state.debug.drain_var_writes().is_empty(), "{field}");
    }

    fs::remove_dir_all(root).ok();
}
