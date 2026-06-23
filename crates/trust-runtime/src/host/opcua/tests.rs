use super::*;
use crate::value::{EnumValue, Value};
use std::sync::Mutex;

static OPCUA_CLIENT_PKI_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn maps_scalar_numeric_and_string_types() {
    assert_eq!(
        map_iec_value(&Value::Bool(true)),
        Some(OpcUaValue {
            data_type: OpcUaDataType::Boolean,
            value: OpcUaVariant::Boolean(true),
        })
    );
    assert_eq!(
        map_iec_value(&Value::DInt(42)),
        Some(OpcUaValue {
            data_type: OpcUaDataType::Int32,
            value: OpcUaVariant::Int32(42),
        })
    );
    assert_eq!(
        map_iec_value(&Value::LReal(3.5)),
        Some(OpcUaValue {
            data_type: OpcUaDataType::Double,
            value: OpcUaVariant::Double(3.5),
        })
    );
    assert_eq!(
        map_iec_value(&Value::String(smol_str::SmolStr::new("Pump"))),
        Some(OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String("Pump".to_string()),
        })
    );
}

#[test]
fn maps_enum_values_as_string_variants() {
    let mut registry = trust_hir::types::TypeRegistry::new();
    let quality = registry.register_enum(
        "ADS_QUALITY",
        trust_hir::TypeId::INT,
        vec![
            (smol_str::SmolStr::new("Stale"), 0),
            (smol_str::SmolStr::new("Good"), 1),
        ],
    );
    let value = Value::Enum(Box::new(
        EnumValue::new(&registry, quality, "Good").expect("enum value"),
    ));

    assert_eq!(
        map_iec_value(&value),
        Some(OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String("Good".to_string()),
        })
    );
}

#[test]
fn rejects_non_scalar_or_protocol_specific_types() {
    assert!(map_iec_value(&Value::Null).is_none());
    assert!(map_iec_value(&Value::Reference(None)).is_none());
    assert!(map_iec_value(&Value::Time(crate::value::Duration::from_millis(10))).is_none());
}

#[test]
fn secure_profile_defaults_to_signed_and_encrypted_policy() {
    assert_eq!(
        OpcUaSecurityProfile::default(),
        OpcUaSecurityProfile {
            policy: OpcUaSecurityPolicy::Basic256Sha256,
            mode: OpcUaMessageSecurityMode::SignAndEncrypt,
            allow_anonymous: false,
        }
    );
}

#[test]
fn parses_security_policy_and_mode_aliases() {
    assert_eq!(
        OpcUaSecurityPolicy::parse("basic256_sha256"),
        Some(OpcUaSecurityPolicy::Basic256Sha256)
    );
    assert_eq!(
        OpcUaSecurityPolicy::parse("Aes128-Sha256-RsaOaep"),
        Some(OpcUaSecurityPolicy::Aes128Sha256RsaOaep)
    );
    assert_eq!(
        OpcUaMessageSecurityMode::parse("sign_and_encrypt"),
        Some(OpcUaMessageSecurityMode::SignAndEncrypt)
    );
    assert_eq!(
        OpcUaMessageSecurityMode::parse("none"),
        Some(OpcUaMessageSecurityMode::None)
    );
}

#[test]
fn rejects_invalid_security_profile_combinations() {
    let invalid = OpcUaSecurityProfile {
        policy: OpcUaSecurityPolicy::None,
        mode: OpcUaMessageSecurityMode::Sign,
        allow_anonymous: true,
    };
    assert!(validate_security_profile(&invalid).is_err());
}

#[test]
fn opcua_client_trust_store_can_be_listed_and_cleared() {
    let _guard = OPCUA_CLIENT_PKI_ENV_LOCK
        .lock()
        .expect("OPC UA client PKI env lock");
    let root = std::env::temp_dir().join(format!(
        "trust-opcua-client-pki-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let trusted = root.join("trusted").join("certs");
    std::fs::create_dir_all(&trusted).expect("create trusted cert dir");
    std::fs::write(trusted.join("server.der"), b"cert").expect("write trusted cert");
    std::env::set_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &root);

    let listed =
        list_trusted_opcua_client_server_certificates().expect("list trusted OPC UA certs");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].file_name, "server.der");

    let cleared =
        clear_trusted_opcua_client_server_certificates().expect("clear trusted OPC UA certs");
    assert_eq!(cleared, 1);
    assert!(list_trusted_opcua_client_server_certificates()
        .expect("list after clear")
        .is_empty());

    std::env::remove_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR");
    let _ = std::fs::remove_dir_all(root);
}
