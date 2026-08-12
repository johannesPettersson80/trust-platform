use serde_json::json;

use super::*;

fn error(result: Result<impl Sized, String>) -> String {
    result.err().expect("validation must reject")
}

#[test]
fn config_validation_contract_type_and_value_errors_are_stable() {
    assert_eq!(
        config_type_error("web.enabled", "boolean"),
        "invalid config value for 'web.enabled': expected boolean"
    );
    assert_eq!(
        config_value_error("web.listen", "must not be empty"),
        "invalid config value for 'web.listen': must not be empty"
    );
}

#[test]
fn config_validation_contract_boolean_accepts_only_json_booleans() {
    assert_eq!(expect_bool("key", &json!(true)), Ok(true));
    assert_eq!(expect_bool("key", &json!(false)), Ok(false));
    for value in [
        json!(0),
        json!(1),
        json!("true"),
        json!(null),
        json!([]),
        json!({}),
    ] {
        assert_eq!(
            error(expect_bool("key", &value)),
            "invalid config value for 'key': expected boolean",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_nonempty_string_trims_unicode_whitespace() {
    assert_eq!(
        expect_non_empty_string("key", &json!("\u{2003} value \u{2002}")),
        Ok("value")
    );
}

#[test]
fn config_validation_contract_nonempty_string_rejects_empty_after_trim() {
    for value in [json!(""), json!("   "), json!("\n\t"), json!("\u{2003}")] {
        assert_eq!(
            error(expect_non_empty_string("key", &value)),
            "invalid config value for 'key': must not be empty",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_nonempty_string_rejects_other_json_types() {
    for value in [json!(true), json!(1), json!(null), json!([]), json!({})] {
        assert_eq!(
            error(expect_non_empty_string("key", &value)),
            "invalid config value for 'key': expected string",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_positive_integer_accepts_signed_bounds() {
    assert_eq!(expect_positive_i64("key", &json!(1)), Ok(1));
    assert_eq!(expect_positive_i64("key", &json!(i64::MAX)), Ok(i64::MAX));
}

#[test]
fn config_validation_contract_positive_integer_rejects_zero_and_negative() {
    for value in [json!(0), json!(-1), json!(i64::MIN)] {
        assert_eq!(
            error(expect_positive_i64("key", &value)),
            "invalid config value for 'key': must be >= 1",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_positive_integer_rejects_above_i64_max() {
    let value = serde_json::Value::Number(serde_json::Number::from(i64::MAX as u64 + 1));
    assert_eq!(
        error(expect_positive_i64("key", &value)),
        "invalid config value for 'key': expected integer >= 1"
    );
}

#[test]
fn config_validation_contract_positive_integer_rejects_fractional_number() {
    assert_eq!(
        error(expect_positive_i64("key", &json!(1.5))),
        "invalid config value for 'key': expected integer >= 1"
    );
}

#[test]
fn config_validation_contract_positive_integer_rejects_non_numbers() {
    for value in [json!(true), json!("1"), json!(null), json!([]), json!({})] {
        assert_eq!(
            error(expect_positive_i64("key", &value)),
            "invalid config value for 'key': expected integer >= 1",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_string_array_accepts_empty_array() {
    assert_eq!(
        expect_string_array("mesh.connect", &json!([])),
        Ok(Vec::new())
    );
}

#[test]
fn config_validation_contract_string_array_trims_and_preserves_order() {
    assert_eq!(
        expect_string_array(
            "mesh.connect",
            &json!(["  peer-b  ", "\tpeer-a\n", "peer-b"])
        ),
        Ok(vec![
            "peer-b".to_string(),
            "peer-a".to_string(),
            "peer-b".to_string(),
        ])
    );
}

#[test]
fn config_validation_contract_string_array_rejects_non_array() {
    for value in [json!(null), json!("peer"), json!({}), json!(1)] {
        assert_eq!(
            error(expect_string_array("mesh.connect", &value)),
            "invalid config value for 'mesh.connect': expected array of strings",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_string_array_reports_non_string_index() {
    assert_eq!(
        error(expect_string_array(
            "mesh.connect",
            &json!(["peer", 42, "other"])
        )),
        "invalid config value for 'mesh.connect': entry 1 must be a string"
    );
}

#[test]
fn config_validation_contract_string_array_reports_empty_index() {
    assert_eq!(
        error(expect_string_array(
            "mesh.connect",
            &json!(["peer", " \t "])
        )),
        "invalid config value for 'mesh.connect': entry 1 must not be empty"
    );
}

#[test]
fn config_validation_contract_string_map_accepts_empty_object() {
    assert_eq!(
        expect_string_map("mesh.subscribe", &json!({})),
        Ok(Vec::new())
    );
}

#[test]
fn config_validation_contract_string_map_trims_keys_and_values() {
    assert_eq!(
        expect_string_map(
            "mesh.subscribe",
            &json!({
                " topic/b ": " TargetB ",
                "topic/a": "\tTargetA\n",
            })
        ),
        Ok(vec![
            ("topic/a".to_string(), "TargetA".to_string()),
            ("topic/b".to_string(), "TargetB".to_string()),
        ])
    );
}

#[test]
fn config_validation_contract_string_map_rejects_non_object() {
    for value in [json!(null), json!([]), json!("map"), json!(1)] {
        assert_eq!(
            error(expect_string_map("mesh.subscribe", &value)),
            "invalid config value for 'mesh.subscribe': expected object of strings",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_string_map_rejects_empty_trimmed_key() {
    assert_eq!(
        error(expect_string_map(
            "mesh.subscribe",
            &json!({"   ": "target"})
        )),
        "invalid config value for 'mesh.subscribe': map keys must not be empty"
    );
}

#[test]
fn config_validation_contract_string_map_rejects_non_string_value() {
    assert_eq!(
        error(expect_string_map("mesh.subscribe", &json!({"topic": 42}))),
        "invalid config value for 'mesh.subscribe': entry 'topic' must be a string"
    );
}

#[test]
fn config_validation_contract_string_map_rejects_empty_trimmed_value() {
    assert_eq!(
        error(expect_string_map(
            "mesh.subscribe",
            &json!({"topic": " \t "})
        )),
        "invalid config value for 'mesh.subscribe': entry 'topic' must not be empty"
    );
}

#[test]
fn config_validation_contract_wan_rules_accept_empty_array() {
    assert_eq!(
        expect_wan_allow_write_rules("runtime_cloud.wan.allow_write", &json!([])),
        Ok(Vec::new())
    );
}

#[test]
fn config_validation_contract_wan_rules_trim_and_preserve_order() {
    let rules = expect_wan_allow_write_rules(
        "runtime_cloud.wan.allow_write",
        &json!([
            {"action": " write ", "target": " runtime-b "},
            {"action": "force", "target": "runtime-c"},
        ]),
    )
    .expect("valid WAN rules");
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].action, "write");
    assert_eq!(rules[0].target, "runtime-b");
    assert_eq!(rules[1].action, "force");
    assert_eq!(rules[1].target, "runtime-c");
}

#[test]
fn config_validation_contract_wan_rules_reject_non_array() {
    for value in [json!(null), json!({}), json!("rules"), json!(1)] {
        assert_eq!(
            error(expect_wan_allow_write_rules(
                "runtime_cloud.wan.allow_write",
                &value
            )),
            "invalid config value for 'runtime_cloud.wan.allow_write': expected array of {action,target} objects",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_wan_rules_report_non_object_index() {
    assert_eq!(
        error(expect_wan_allow_write_rules(
            "runtime_cloud.wan.allow_write",
            &json!([{"action": "write", "target": "a"}, "invalid"])
        )),
        "invalid config value for 'runtime_cloud.wan.allow_write': entry 1 must be an object"
    );
}

#[test]
fn config_validation_contract_wan_rules_require_nonempty_action() {
    for value in [
        json!([{"target": "runtime"}]),
        json!([{"action": null, "target": "runtime"}]),
        json!([{"action": 1, "target": "runtime"}]),
        json!([{"action": " ", "target": "runtime"}]),
    ] {
        assert_eq!(
            error(expect_wan_allow_write_rules(
                "runtime_cloud.wan.allow_write",
                &value
            )),
            "invalid config value for 'runtime_cloud.wan.allow_write': entry 0 requires action",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_wan_rules_require_nonempty_target() {
    for value in [
        json!([{"action": "write"}]),
        json!([{"action": "write", "target": null}]),
        json!([{"action": "write", "target": 1}]),
        json!([{"action": "write", "target": " "}]),
    ] {
        assert_eq!(
            error(expect_wan_allow_write_rules(
                "runtime_cloud.wan.allow_write",
                &value
            )),
            "invalid config value for 'runtime_cloud.wan.allow_write': entry 0 requires target",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_wan_rules_reject_unknown_fields() {
    let value = json!([
        {"action": "write", "target": "runtime", "secret": "must-not-be-ignored"}
    ]);
    let message = error(expect_wan_allow_write_rules(
        "runtime_cloud.wan.allow_write",
        &value,
    ));
    assert!(message.contains("entry 0"), "{message}");
    assert!(message.contains("unknown field 'secret'"), "{message}");
    assert!(!message.contains("must-not-be-ignored"), "{message}");
}

#[test]
fn config_validation_contract_link_rules_accept_empty_array() {
    assert_eq!(
        expect_link_preference_rules("runtime_cloud.links.transports", &json!([])),
        Ok(Vec::new())
    );
}

#[test]
fn config_validation_contract_link_rules_trim_and_preserve_order() {
    let rules = expect_link_preference_rules(
        "runtime_cloud.links.transports",
        &json!([
            {"source": " a ", "target": " b ", "transport": " MODBUS_TCP "},
            {"source": "b", "target": "c", "transport": "mqtt"},
        ]),
    )
    .expect("valid link rules");
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].source, "a");
    assert_eq!(rules[0].target, "b");
    assert_eq!(rules[0].transport.as_str(), "modbus-tcp");
    assert_eq!(rules[1].transport.as_str(), "mqtt");
}

#[test]
fn config_validation_contract_link_rules_reject_non_array() {
    for value in [json!(null), json!({}), json!("rules"), json!(1)] {
        assert_eq!(
            error(expect_link_preference_rules(
                "runtime_cloud.links.transports",
                &value
            )),
            "invalid config value for 'runtime_cloud.links.transports': expected array of {source,target,transport} objects",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_link_rules_report_non_object_index() {
    assert_eq!(
        error(expect_link_preference_rules(
            "runtime_cloud.links.transports",
            &json!([
                {"source": "a", "target": "b", "transport": "mesh"},
                false
            ])
        )),
        "invalid config value for 'runtime_cloud.links.transports': entry 1 must be an object"
    );
}

#[test]
fn config_validation_contract_link_rules_require_nonempty_source() {
    for value in [
        json!([{"target": "b", "transport": "mesh"}]),
        json!([{"source": null, "target": "b", "transport": "mesh"}]),
        json!([{"source": 1, "target": "b", "transport": "mesh"}]),
        json!([{"source": " ", "target": "b", "transport": "mesh"}]),
    ] {
        assert_eq!(
            error(expect_link_preference_rules(
                "runtime_cloud.links.transports",
                &value
            )),
            "invalid config value for 'runtime_cloud.links.transports': entry 0 requires source",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_link_rules_require_nonempty_target() {
    for value in [
        json!([{"source": "a", "transport": "mesh"}]),
        json!([{"source": "a", "target": null, "transport": "mesh"}]),
        json!([{"source": "a", "target": 1, "transport": "mesh"}]),
        json!([{"source": "a", "target": " ", "transport": "mesh"}]),
    ] {
        assert_eq!(
            error(expect_link_preference_rules(
                "runtime_cloud.links.transports",
                &value
            )),
            "invalid config value for 'runtime_cloud.links.transports': entry 0 requires target",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_link_rules_require_nonempty_transport() {
    for value in [
        json!([{"source": "a", "target": "b"}]),
        json!([{"source": "a", "target": "b", "transport": null}]),
        json!([{"source": "a", "target": "b", "transport": 1}]),
        json!([{"source": "a", "target": "b", "transport": " "}]),
    ] {
        assert_eq!(
            error(expect_link_preference_rules(
                "runtime_cloud.links.transports",
                &value
            )),
            "invalid config value for 'runtime_cloud.links.transports': entry 0 requires transport",
            "{value}"
        );
    }
}

#[test]
fn config_validation_contract_link_rules_reject_unknown_transport() {
    let message = error(expect_link_preference_rules(
        "runtime_cloud.links.transports",
        &json!([{"source": "a", "target": "b", "transport": "udp"}]),
    ));
    assert!(
        message.contains("invalid runtime.cloud.links.transports[].transport 'udp'"),
        "{message}"
    );
}

#[test]
fn config_validation_contract_link_transport_vocabulary_is_complete() {
    for (source, canonical) in [
        ("realtime", "realtime"),
        ("zenoh", "zenoh"),
        ("mesh", "mesh"),
        ("mqtt", "mqtt"),
        ("modbus-tcp", "modbus-tcp"),
        ("modbus_tcp", "modbus-tcp"),
        ("opcua", "opcua"),
        ("discovery", "discovery"),
        ("web", "web"),
    ] {
        let rules = expect_link_preference_rules(
            "runtime_cloud.links.transports",
            &json!([{"source": "a", "target": "b", "transport": source}]),
        )
        .expect("accepted transport");
        assert_eq!(rules[0].transport.as_str(), canonical, "{source}");
    }
}

#[test]
fn config_validation_contract_link_rules_reject_unknown_fields() {
    let value = json!([
        {
            "source": "a",
            "target": "b",
            "transport": "mesh",
            "password": "must-not-be-ignored"
        }
    ]);
    let message = error(expect_link_preference_rules(
        "runtime_cloud.links.transports",
        &value,
    ));
    assert!(message.contains("entry 0"), "{message}");
    assert!(message.contains("unknown field 'password'"), "{message}");
    assert!(!message.contains("must-not-be-ignored"), "{message}");
}
