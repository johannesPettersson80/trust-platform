use super::*;
use crate::value::Value;

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
