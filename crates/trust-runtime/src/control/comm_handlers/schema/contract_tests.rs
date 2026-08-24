use std::collections::BTreeSet;

use serde_json::{json, Value as JsonValue};

use super::*;
use crate::control::comm_handlers::apply::{
    validate_runtime_file_fields_for_contract, validate_schema_fields_for_contract,
};

#[path = "contract_tests/authority.rs"]
mod authority;
#[path = "contract_tests/instance_projection.rs"]
mod instance_projection;

use authority::{
    protocol_field_ids, ACTION_VOCABULARY, DIRECT_RUNTIME_PROTOCOL_IDS, FIELD_TYPE_VOCABULARY,
    IO_PROTOCOL_IDS, PROTOCOL_IDS,
};

fn protocols() -> Vec<CommProtocolSchema> {
    communication_protocol_schemas(&[])
}

fn protocol<'a>(protocols: &'a [CommProtocolSchema], protocol_id: &str) -> &'a CommProtocolSchema {
    protocols
        .iter()
        .find(|protocol| protocol.id == protocol_id)
        .unwrap_or_else(|| panic!("missing protocol {protocol_id}"))
}

fn field_by_id<'a>(protocol: &'a CommProtocolSchema, field_id: &str) -> &'a CommFieldSchema {
    protocol
        .fields
        .iter()
        .find(|field| field.id == field_id)
        .unwrap_or_else(|| panic!("missing {}.{field_id}", protocol.id))
}

fn defaults_as_toml(protocol: &CommProtocolSchema) -> toml::map::Map<String, toml::Value> {
    protocol
        .fields
        .iter()
        .filter(|field| !field.default.is_null())
        .map(|field| {
            (
                field.id.to_string(),
                json_to_toml(&field.default, protocol.id, field.id),
            )
        })
        .collect()
}

fn json_to_toml(value: &JsonValue, protocol: &str, field: &str) -> toml::Value {
    match value {
        JsonValue::Bool(value) => toml::Value::Boolean(*value),
        JsonValue::Number(value) => toml::Value::Integer(
            value
                .as_i64()
                .unwrap_or_else(|| panic!("{protocol}.{field} default is not an integer")),
        ),
        JsonValue::String(value) => toml::Value::String(value.clone()),
        JsonValue::Array(values) => toml::Value::Array(
            values
                .iter()
                .map(|value| json_to_toml(value, protocol, field))
                .collect(),
        ),
        JsonValue::Object(values) => toml::Value::Table(
            values
                .iter()
                .map(|(key, value)| (key.clone(), json_to_toml(value, protocol, field)))
                .collect(),
        ),
        JsonValue::Null => panic!("{protocol}.{field} null must be omitted from TOML defaults"),
    }
}

fn validate_defaults(
    protocol: &CommProtocolSchema,
    params: &toml::map::Map<String, toml::Value>,
) -> Vec<crate::control::comm_handlers::apply::CommFieldError> {
    if IO_PROTOCOL_IDS.contains(&protocol.id) {
        validate_schema_fields_for_contract(protocol.id, &toml::Value::Table(params.clone()))
    } else if DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id) {
        validate_runtime_file_fields_for_contract(protocol.id, params)
    } else {
        panic!("{} has no direct default-validation route", protocol.id)
    }
}

fn errors_contain_field(
    errors: &[crate::control::comm_handlers::apply::CommFieldError],
    field_id: &str,
) -> bool {
    errors.iter().any(|error| error.field == field_id)
}

#[test]
fn registry_has_exact_protocol_order_and_denominator() {
    let actual: Vec<_> = protocols()
        .into_iter()
        .map(|protocol| protocol.id)
        .collect();
    assert_eq!(actual, PROTOCOL_IDS);
}

#[test]
fn protocol_ids_are_unique_and_canonical() {
    let protocols = protocols();
    let unique: BTreeSet<_> = protocols.iter().map(|protocol| protocol.id).collect();
    assert_eq!(unique.len(), protocols.len());
    for protocol in protocols {
        assert_eq!(normalize_protocol(protocol.id), protocol.id);
        assert!(!protocol.id.is_empty());
    }
}

#[test]
fn protocol_metadata_is_complete() {
    for protocol in protocols() {
        for (name, value) in [
            ("title", protocol.title),
            ("purpose", protocol.purpose),
            ("availability", protocol.availability),
            ("category", protocol.category),
            ("config_home", protocol.config_home),
            ("apply_mode", protocol.apply_mode),
            ("lifecycle_effect", protocol.lifecycle_effect),
        ] {
            assert!(!value.trim().is_empty(), "{} has blank {name}", protocol.id);
        }
    }
}

#[test]
fn protocol_categories_are_unique_and_contain_primary_category() {
    for protocol in protocols() {
        let unique: BTreeSet<_> = protocol.categories.iter().copied().collect();
        assert_eq!(
            unique.len(),
            protocol.categories.len(),
            "{} has duplicate categories",
            protocol.id
        );
        assert!(
            protocol.categories.contains(&protocol.category),
            "{} categories omit primary category {}",
            protocol.id,
            protocol.category
        );
        assert!(protocol.categories.iter().all(|value| !value.is_empty()));
    }
}

#[test]
fn configuration_homes_match_apply_ownership() {
    let protocols = protocols();
    for protocol_id in IO_PROTOCOL_IDS {
        assert_eq!(protocol(&protocols, protocol_id).config_home, "io.toml");
    }
    for protocol_id in DIRECT_RUNTIME_PROTOCOL_IDS {
        assert_eq!(
            protocol(&protocols, protocol_id).config_home,
            "runtime.toml"
        );
    }
    assert_eq!(
        protocol(&protocols, "opcua_client").config_home,
        "opcua_client.toml"
    );
    assert_eq!(protocol(&protocols, "ads").config_home, "ads.toml");
}

#[test]
fn protocol_apply_and_lifecycle_vocabularies_are_closed() {
    for protocol in protocols() {
        assert_eq!(protocol.availability, "default");
        assert_eq!(protocol.apply_mode, "file");
        assert_eq!(protocol.lifecycle_effect, "restart_required");
    }
}

#[test]
fn io_protocol_driver_mapping_round_trips() {
    let protocols = protocols();
    for protocol_id in IO_PROTOCOL_IDS {
        let protocol = protocol(&protocols, protocol_id);
        assert_eq!(protocol_to_driver(protocol.id), Some(protocol.driver));
        assert_eq!(driver_to_protocol(protocol.driver), Some(protocol.id));
    }
}

#[test]
fn protocol_and_driver_aliases_normalize_deterministically() {
    for (input, expected) in [
        (" MODBUS-TCP ", "modbus_tcp"),
        ("ModBus_Tcp", "modbus_tcp"),
        (" MQTT ", "mqtt"),
        ("REALTIME-T0", "realtime_t0"),
        (" runtime-cloud ", "runtime_cloud"),
    ] {
        assert_eq!(normalize_protocol(input), expected);
    }
    for alias in ["modbus-tcp", "modbus_tcp"] {
        assert_eq!(driver_to_protocol(alias), Some("modbus_tcp"));
    }
    for alias in ["simulated", "sim", "noop"] {
        assert_eq!(driver_to_protocol(alias), Some("simulated"));
    }
}

#[test]
fn every_advertised_protocol_has_an_apply_route() {
    for protocol in protocols() {
        let routed = protocol_to_driver(protocol.id).is_some()
            || supports_runtime_file_protocol(protocol.id)
            || matches!(protocol.id, "ads" | "ads_server");
        assert!(routed, "{} has no apply route", protocol.id);
    }
}

#[test]
fn runtime_file_route_registry_is_exact() {
    for protocol_id in PROTOCOL_IDS {
        let expected = matches!(
            protocol_id,
            "opcua"
                | "opcua_client"
                | "openot"
                | "discovery"
                | "mesh"
                | "realtime_t0"
                | "runtime_cloud"
        );
        assert_eq!(
            supports_runtime_file_protocol(protocol_id),
            expected,
            "runtime-file route drift for {protocol_id}"
        );
    }
}

#[test]
fn protocol_filter_returns_exact_requested_schema() {
    for protocol_id in PROTOCOL_IDS {
        let value = static_comm_schema_value(Some(&json!({ "protocol": protocol_id })))
            .unwrap_or_else(|error| panic!("schema filter {protocol_id}: {error}"));
        let filtered = value["protocols"].as_array().expect("protocol array");
        assert_eq!(filtered.len(), 1, "filter result for {protocol_id}");
        assert_eq!(filtered[0]["id"], protocol_id);
    }
}

#[test]
fn protocol_filter_accepts_case_space_and_hyphen_aliases() {
    for (filter, expected) in [
        (" MODBUS-TCP ", "modbus_tcp"),
        (" OpcUa-Client ", "opcua_client"),
        (" REALTIME-T0 ", "realtime_t0"),
        (" RUNTIME-CLOUD ", "runtime_cloud"),
    ] {
        let value = static_comm_schema_value(Some(&json!({ "protocol": filter })))
            .unwrap_or_else(|error| panic!("schema filter {filter}: {error}"));
        let filtered = value["protocols"].as_array().expect("protocol array");
        assert_eq!(filtered.len(), 1, "filter result for {filter}");
        assert_eq!(filtered[0]["id"], expected);
    }
}

#[test]
fn unknown_protocol_filter_returns_empty_registry() {
    let value = static_comm_schema_value(Some(&json!({ "protocol": "not-a-protocol" })))
        .expect("unknown string filter remains a valid query");
    assert_eq!(value["protocols"], json!([]));
}

#[test]
fn non_string_protocol_filter_is_rejected_instead_of_becoming_unfiltered() {
    for malformed in [JsonValue::Null, json!(7), json!(true), json!([]), json!({})] {
        let params = json!({ "protocol": malformed });
        assert!(
            static_comm_schema_value(Some(&params)).is_err(),
            "malformed filter returned the full registry: {params}"
        );
    }
}

#[test]
fn protocol_field_identities_match_the_authoritative_denominator() {
    for protocol in protocols() {
        let actual: Vec<_> = protocol.fields.iter().map(|field| field.id).collect();
        assert_eq!(
            actual,
            protocol_field_ids(protocol.id),
            "{} field identity/order drift",
            protocol.id
        );
    }
}

#[test]
fn field_ids_are_unique_within_each_protocol() {
    for protocol in protocols() {
        let unique: BTreeSet<_> = protocol.fields.iter().map(|field| field.id).collect();
        assert_eq!(
            unique.len(),
            protocol.fields.len(),
            "{} has duplicate field IDs",
            protocol.id
        );
        assert!(protocol.fields.iter().all(|field| !field.id.is_empty()));
    }
}

#[test]
fn field_labels_and_help_are_complete() {
    for protocol in protocols() {
        for field in protocol.fields {
            assert!(
                !field.label.trim().is_empty(),
                "{}.{} has blank label",
                protocol.id,
                field.id
            );
            assert!(
                !field.help.trim().is_empty(),
                "{}.{} has blank help",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn field_type_vocabulary_is_closed() {
    for protocol in protocols() {
        for field in protocol.fields {
            assert!(
                FIELD_TYPE_VOCABULARY.contains(&field.field_type),
                "{}.{} has unknown field type {}",
                protocol.id,
                field.id,
                field.field_type
            );
        }
    }
}

#[test]
fn field_default_shapes_match_declared_types() {
    for protocol in protocols() {
        for field in protocol.fields {
            let shape_matches = match field.field_type {
                "string" | "enum" | "endpoint" | "path" => field.default.is_string(),
                "secret" => field.default.is_null(),
                "boolean" => field.default.is_boolean(),
                "number" => field.default.as_i64().is_some(),
                "json_array" => field.default.is_array(),
                "json_object" => field.default.is_object(),
                _ => false,
            };
            assert!(
                shape_matches,
                "{}.{} default shape contradicts {}",
                protocol.id, field.id, field.field_type
            );
        }
    }
}

#[test]
fn required_fields_have_non_null_defaults() {
    for protocol in protocols() {
        for field in protocol.fields.iter().filter(|field| field.required) {
            assert!(
                !field.default.is_null(),
                "{}.{} is required but has a null default",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn secret_fields_are_exact_optional_null_typed_set() {
    let protocols = protocols();
    let actual: BTreeSet<_> = protocols
        .iter()
        .flat_map(|protocol| {
            protocol
                .fields
                .iter()
                .filter(|field| field.secret)
                .map(move |field| format!("{}.{}", protocol.id, field.id))
        })
        .collect();
    let expected = BTreeSet::from([
        "mesh.auth_token".to_string(),
        "mqtt.password".to_string(),
        "opcua.password".to_string(),
    ]);
    assert_eq!(actual, expected);

    for protocol in protocols {
        for field in protocol.fields {
            assert_eq!(
                field.secret,
                field.field_type == "secret",
                "{}.{} secret flag/type drift",
                protocol.id,
                field.id
            );
            if field.secret {
                assert!(field.default.is_null());
                assert!(!field.required);
                assert!(field.options.is_none());
                assert!(field.validation.is_none());
            }
        }
    }
}

#[test]
fn serialized_static_schema_contains_no_secret_value() {
    let value = static_comm_schema_value(None).expect("static communication schema");
    let text = serde_json::to_string(&value).expect("serialize communication schema");
    for forbidden in [
        "\"password\":\"",
        "\"auth_token\":\"",
        "\"private_key\":\"",
        "\"client_key\":\"",
    ] {
        assert!(
            !text.contains(forbidden),
            "static schema contains secret material matching {forbidden}"
        );
    }
}

#[test]
fn enum_options_are_unique_nonempty_and_contain_default() {
    for protocol in protocols() {
        for field in protocol.fields {
            if field.field_type == "enum" {
                let options = field
                    .options
                    .as_ref()
                    .unwrap_or_else(|| panic!("{}.{} enum has no options", protocol.id, field.id));
                let unique: BTreeSet<_> = options.iter().copied().collect();
                assert!(
                    !options.is_empty(),
                    "{}.{} has no options",
                    protocol.id,
                    field.id
                );
                assert_eq!(
                    unique.len(),
                    options.len(),
                    "{}.{} has duplicate options",
                    protocol.id,
                    field.id
                );
                let default = field.default.as_str().expect("enum default string");
                assert!(
                    options.contains(&default),
                    "{}.{} default {default} is not an option",
                    protocol.id,
                    field.id
                );
            } else {
                assert!(
                    field.options.is_none(),
                    "{}.{} has options but is not an enum",
                    protocol.id,
                    field.id
                );
            }
        }
    }
}

#[test]
fn number_validation_metadata_is_complete_and_bounded() {
    for protocol in protocols() {
        for field in protocol.fields {
            if field.field_type != "number" {
                continue;
            }
            let validation = field
                .validation
                .as_ref()
                .unwrap_or_else(|| panic!("{}.{} has no validation", protocol.id, field.id));
            assert_eq!(validation["kind"], "integer_range");
            let min = validation["min"]
                .as_i64()
                .unwrap_or_else(|| panic!("{}.{} has no integer min", protocol.id, field.id));
            let max = validation["max"]
                .as_i64()
                .unwrap_or_else(|| panic!("{}.{} has no integer max", protocol.id, field.id));
            let default = field.default.as_i64().expect("integer default");
            assert!(
                min <= max,
                "{}.{} has inverted range",
                protocol.id,
                field.id
            );
            assert!(
                (min..=max).contains(&default),
                "{}.{} default {default} outside {min}..={max}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn endpoint_validation_metadata_uses_closed_kinds() {
    for protocol in protocols() {
        for field in protocol.fields {
            if field.field_type != "endpoint" {
                continue;
            }
            let kind = field
                .validation
                .as_ref()
                .and_then(|value| value.get("kind"))
                .and_then(JsonValue::as_str)
                .unwrap_or_else(|| panic!("{}.{} has no endpoint kind", protocol.id, field.id));
            assert!(
                matches!(kind, "host_port" | "socket_addr"),
                "{}.{} has unknown endpoint validation {kind}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn validation_metadata_appears_only_on_number_and_endpoint_fields() {
    for protocol in protocols() {
        for field in protocol.fields {
            assert_eq!(
                field.validation.is_some(),
                matches!(field.field_type, "number" | "endpoint"),
                "{}.{} validation/type drift",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn visibility_predicates_reference_valid_controlling_fields_and_values() {
    for protocol in protocols() {
        for field in &protocol.fields {
            let Some(predicate) = &field.visible_when else {
                continue;
            };
            let controlling_id = predicate["field"].as_str().unwrap_or_else(|| {
                panic!(
                    "{}.{} visibility field is not a string",
                    protocol.id, field.id
                )
            });
            let controlling = field_by_id(&protocol, controlling_id);
            let equals = &predicate["equals"];
            assert!(
                equals.is_string() || equals.is_boolean() || equals.is_number(),
                "{}.{} visibility equality has unsupported shape",
                protocol.id,
                field.id
            );
            if controlling.field_type == "enum" {
                let equals = equals.as_str().expect("enum visibility string");
                assert!(
                    controlling
                        .options
                        .as_ref()
                        .is_some_and(|options| options.contains(&equals)),
                    "{}.{} visibility value {equals} is not valid for {controlling_id}",
                    protocol.id,
                    field.id
                );
            }
        }
    }
}

#[test]
fn gpio_visibility_predicates_are_the_exact_registered_set() {
    let protocols = protocols();
    let gpio = protocol(&protocols, "gpio");
    assert_eq!(
        field_by_id(gpio, "chip").visible_when,
        Some(json!({ "field": "backend", "equals": "libgpiod" }))
    );
    assert_eq!(
        field_by_id(gpio, "sysfs_base").visible_when,
        Some(json!({ "field": "backend", "equals": "sysfs" }))
    );
    let visible_count = protocols
        .iter()
        .flat_map(|protocol| &protocol.fields)
        .filter(|field| field.visible_when.is_some())
        .count();
    assert_eq!(visible_count, 2);
}

#[test]
fn actions_are_unique_and_use_the_closed_vocabulary() {
    for protocol in protocols() {
        let unique: BTreeSet<_> = protocol.actions.iter().copied().collect();
        assert_eq!(
            unique.len(),
            protocol.actions.len(),
            "{} has duplicate actions",
            protocol.id
        );
        assert!(
            !protocol.actions.is_empty(),
            "{} has no actions",
            protocol.id
        );
        for action in protocol.actions {
            assert!(
                ACTION_VOCABULARY.contains(&action),
                "{} advertises unknown action {action}",
                protocol.id
            );
        }
    }
}

#[test]
fn supports_test_and_test_action_are_equivalent() {
    for protocol in protocols() {
        assert_eq!(
            protocol.supports_test,
            protocol.actions.contains(&"test"),
            "{} exposes contradictory Test capability metadata",
            protocol.id
        );
    }
}

#[test]
fn multi_instance_metadata_matches_configuration_shape() {
    let protocols = protocols();
    for protocol_id in [
        "modbus_tcp",
        "mqtt",
        "ethercat",
        "gpio",
        "simulated",
        "loopback",
        "opcua_client",
        "ads",
    ] {
        assert!(
            protocol(&protocols, protocol_id).supports_multi_instance,
            "{protocol_id} must support multiple instances"
        );
    }
    for protocol_id in [
        "opcua",
        "openot",
        "discovery",
        "mesh",
        "realtime_t0",
        "runtime_cloud",
        "ads_server",
    ] {
        assert!(
            !protocol(&protocols, protocol_id).supports_multi_instance,
            "{protocol_id} is a singleton runtime section"
        );
    }
}

#[test]
fn directly_applicable_schema_defaults_pass_the_owning_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        let params = defaults_as_toml(protocol);
        let errors = validate_defaults(protocol, &params);
        assert!(
            errors.is_empty(),
            "{} defaults rejected by apply validator: {errors:?}",
            protocol.id
        );
    }
}

#[test]
fn client_connection_defaults_are_explicitly_incomplete_authoring_seeds() {
    let protocols = protocols();
    for protocol_id in ["ads", "opcua_client"] {
        let protocol = protocol(&protocols, protocol_id);
        assert_eq!(field_by_id(protocol, "enabled").default, json!(true));
        assert_eq!(field_by_id(protocol, "connections").default, json!([]));
        assert!(
            field_by_id(protocol, "connections")
                .help
                .contains("requires at least one connection"),
            "{protocol_id} must explain why its empty default cannot activate"
        );
    }
}

#[test]
fn integer_minimums_are_enforced_by_the_owning_apply_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        for field in protocol
            .fields
            .iter()
            .filter(|field| field.field_type == "number")
        {
            let min = field.validation.as_ref().expect("number validation")["min"]
                .as_i64()
                .expect("integer min");
            let mut params = defaults_as_toml(protocol);
            params.insert(field.id.to_string(), toml::Value::Integer(min - 1));
            let errors = validate_defaults(protocol, &params);
            assert!(
                errors_contain_field(&errors, field.id),
                "{}.{} advertised minimum {min} is not enforced: {errors:?}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn integer_maximums_are_enforced_by_the_owning_apply_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        for field in protocol
            .fields
            .iter()
            .filter(|field| field.field_type == "number")
        {
            let max = field.validation.as_ref().expect("number validation")["max"]
                .as_i64()
                .expect("integer max");
            let mut params = defaults_as_toml(protocol);
            params.insert(field.id.to_string(), toml::Value::Integer(max + 1));
            let errors = validate_defaults(protocol, &params);
            assert!(
                errors_contain_field(&errors, field.id),
                "{}.{} advertised maximum {max} is not enforced: {errors:?}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn enum_options_are_enforced_by_the_owning_apply_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        for field in protocol
            .fields
            .iter()
            .filter(|field| field.field_type == "enum")
        {
            let mut params = defaults_as_toml(protocol);
            params.insert(
                field.id.to_string(),
                toml::Value::String("not-a-schema-option".to_string()),
            );
            let errors = validate_defaults(protocol, &params);
            assert!(
                errors_contain_field(&errors, field.id),
                "{}.{} advertised enum options are not enforced: {errors:?}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn endpoint_shapes_are_enforced_by_the_owning_apply_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        for field in protocol
            .fields
            .iter()
            .filter(|field| field.field_type == "endpoint")
        {
            let mut params = defaults_as_toml(protocol);
            params.insert(
                field.id.to_string(),
                toml::Value::String("not-an-endpoint".to_string()),
            );
            let errors = validate_defaults(protocol, &params);
            assert!(
                errors_contain_field(&errors, field.id),
                "{}.{} endpoint validation is not enforced: {errors:?}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn required_strings_are_enforced_by_the_owning_apply_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        for field in protocol
            .fields
            .iter()
            .filter(|field| field.required && matches!(field.field_type, "string" | "path"))
        {
            let mut params = defaults_as_toml(protocol);
            params.insert(field.id.to_string(), toml::Value::String(" ".to_string()));
            let errors = validate_defaults(protocol, &params);
            assert!(
                errors_contain_field(&errors, field.id),
                "{}.{} is marked required but blank is accepted: {errors:?}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn array_shapes_are_enforced_by_the_owning_apply_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        for field in protocol
            .fields
            .iter()
            .filter(|field| field.field_type == "json_array")
        {
            let mut params = defaults_as_toml(protocol);
            params.insert(
                field.id.to_string(),
                toml::Value::String("not-array".to_string()),
            );
            let errors = validate_defaults(protocol, &params);
            assert!(
                errors_contain_field(&errors, field.id),
                "{}.{} array shape is not enforced: {errors:?}",
                protocol.id,
                field.id
            );
        }
    }
}

#[test]
fn object_shapes_are_enforced_by_the_owning_apply_validator() {
    for protocol in protocols().iter().filter(|protocol| {
        IO_PROTOCOL_IDS.contains(&protocol.id) || DIRECT_RUNTIME_PROTOCOL_IDS.contains(&protocol.id)
    }) {
        for field in protocol
            .fields
            .iter()
            .filter(|field| field.field_type == "json_object")
        {
            let mut params = defaults_as_toml(protocol);
            params.insert(
                field.id.to_string(),
                toml::Value::String("not-object".to_string()),
            );
            let errors = validate_defaults(protocol, &params);
            assert!(
                errors_contain_field(&errors, field.id),
                "{}.{} object shape is not enforced: {errors:?}",
                protocol.id,
                field.id
            );
        }
    }
}
