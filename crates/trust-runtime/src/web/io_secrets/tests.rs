use super::*;

const IO: &str = r#"# Keep this comment
[[io.drivers]]
name = "mqtt"
params = { broker = "127.0.0.1:1883", username = "plant", password = "secret-value", topic_in = "a", topic_out = "b" } # Keep this too
"#;

#[test]
fn redacted_text_round_trip_preserves_comments_and_unrelated_edits() {
    let redacted = redact(IO).unwrap();
    assert!(!redacted.contains("secret-value"));
    assert!(redacted.contains("# Keep this comment"));
    assert!(redacted.contains("# Keep this too"));
    let edited = redacted.replace("topic_in = \"a\"", "topic_in = \"changed\"");
    let saved = restore(&edited, IO).unwrap();
    assert!(saved.contains("secret-value"));
    assert!(saved.contains("changed"));
    assert!(saved.contains("# Keep this too"));
}

#[test]
fn literal_marker_is_a_valid_stored_secret() {
    let stored = IO.replace("secret-value", SECRET_MARKER);
    assert_eq!(restore(&redact(&stored).unwrap(), &stored).unwrap(), stored);
}

#[test]
fn markers_cannot_move_to_a_different_driver_or_be_created_without_storage() {
    let redacted = redact(IO).unwrap();
    assert!(restore(&redacted, "").is_err());
    assert!(restore(
        &redacted.replace("name = \"mqtt\"", "name = \"loopback\""),
        IO
    )
    .is_err());
}

#[test]
fn legacy_and_nested_secret_values_are_redacted() {
    let text = r#"[io]
driver = "mqtt"
[io.params]
password = "secret-value"
peers = [{ AUTH_TOKEN = "nested-value", label = "keep" }]
[io.params.credentials]
user = "private-user"
"#;
    let projected = redact(text).unwrap();
    for secret in ["secret-value", "nested-value", "private-user"] {
        assert!(!projected.contains(secret), "{projected}");
    }
    assert!(projected.contains("keep"));
}

#[test]
fn malformed_io_errors_never_echo_source_credentials() {
    let malformed = "[io]\npassword = 'secret-value' bad syntax";
    assert!(!redact(malformed)
        .unwrap_err()
        .to_string()
        .contains("secret-value"));
    assert!(!restore(malformed, IO)
        .unwrap_err()
        .to_string()
        .contains("secret-value"));
    assert!(!restore(&redact(IO).unwrap(), malformed)
        .unwrap_err()
        .to_string()
        .contains("secret-value"));
}

#[test]
fn valid_replacement_can_repair_malformed_storage_without_markers() {
    assert!(restore(IO, "invalid stored TOML").is_ok());
}

#[test]
fn text_restore_rejects_redirecting_retained_password() {
    for driver in ["mqtt", "mqtt-tcp"] {
        let stored = IO.replace("name = \"mqtt\"", &format!("driver = \"{driver}\""));
        let edited = redact(&stored)
            .unwrap()
            .replace("127.0.0.1:1883", "127.0.0.1:1884");
        let error = restore(&edited, &stored).unwrap_err().to_string();
        assert!(!error.contains("secret-value"));
        assert!(restore(&edited.replace(SECRET_MARKER, "replacement"), &stored).is_ok());
    }
}

#[test]
fn omitted_parameter_tables_remain_absent_in_both_configuration_forms() {
    for text in [
        "[io]\ndriver = \"loopback\"\n",
        "[[io.drivers]]\nname = \"loopback\"\n",
        "[[io.drivers]]\nname = \"mqtt\"\nparams = { broker = \"localhost:1883\", username = \"plant\", password = \"private-value\" }\n[[io.drivers]]\nname = \"loopback\"\n",
    ] {
        let projected = redact(text).unwrap();
        assert_eq!(restore(&projected, text).unwrap(), text);
        assert_eq!(restore(text, "").unwrap(), text);
    }
}
