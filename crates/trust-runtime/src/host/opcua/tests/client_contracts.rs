use super::*;

#[cfg(unix)]
static PKI_PATH_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(unix)]
fn unique_pki_suffix() -> String {
    let sequence = PKI_PATH_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{sequence}-{nanos}", std::process::id())
}

#[test]
fn client_config_rejects_ambiguous_identity_auth_and_access() {
    let cases = [
        (
            "duplicate connection name",
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://127.0.0.1:4840/a"
[[connections.points]]
var = "first"
node_id = "ns=2;i=1"
type = "bool"

[[connections]]
name = " line "
endpoint_url = "opc.tcp://127.0.0.1:4840/b"
[[connections.points]]
var = "second"
node_id = "ns=2;i=2"
type = "bool"
"#,
        ),
        (
            "endpoint without authority",
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://"
[[connections.points]]
var = "first"
node_id = "ns=2;i=1"
type = "bool"
"#,
        ),
        (
            "endpoint query without authority",
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://?query"
[[connections.points]]
var = "first"
node_id = "ns=2;i=1"
type = "bool"
"#,
        ),
        (
            "endpoint fragment without authority",
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://#fragment"
[[connections.points]]
var = "first"
node_id = "ns=2;i=1"
type = "bool"
"#,
        ),
        (
            "anonymous credentials present",
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://127.0.0.1:4840"
auth = "anonymous"
username = ""
[[connections.points]]
var = "first"
node_id = "ns=2;i=1"
type = "bool"
"#,
        ),
        (
            "access and writable both present",
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://127.0.0.1:4840"
[[connections.points]]
var = "first"
node_id = "ns=2;i=1"
type = "bool"
access = "read"
writable = true
"#,
        ),
    ];

    for (case, input) in cases {
        assert!(
            parse_opcua_client_toml(input).is_err(),
            "{case} must fail closed"
        );
    }

    let secret = "sentinel-password-must-not-leak";
    let secret_error = parse_opcua_client_toml(
        format!(
            r#"
[[connections]]
name = "line"
endpoint_url = "opc.tcp://127.0.0.1:4840"
auth = "username"
username = ""
password = "{secret}"
[[connections.points]]
var = "first"
node_id = "ns=2;i=1"
type = "bool"
"#
        )
        .as_str(),
    )
    .expect_err("incomplete username authentication must fail");
    assert!(
        !secret_error.to_string().contains(secret),
        "configuration error exposed the password"
    );
}

#[test]
fn client_config_scalar_type_and_access_aliases_are_stable() {
    for (text, expected) in [
        ("bool", OpcUaDataType::Boolean),
        ("boolean", OpcUaDataType::Boolean),
        ("int", OpcUaDataType::Int16),
        ("int16", OpcUaDataType::Int16),
        ("dint", OpcUaDataType::Int32),
        ("int32", OpcUaDataType::Int32),
        ("lint", OpcUaDataType::Int64),
        ("int64", OpcUaDataType::Int64),
        ("uint", OpcUaDataType::UInt16),
        ("uint16", OpcUaDataType::UInt16),
        ("udint", OpcUaDataType::UInt32),
        ("uint32", OpcUaDataType::UInt32),
        ("ulint", OpcUaDataType::UInt64),
        ("uint64", OpcUaDataType::UInt64),
        ("real", OpcUaDataType::Float),
        ("float", OpcUaDataType::Float),
        ("float32", OpcUaDataType::Float),
        ("lreal", OpcUaDataType::Double),
        ("double", OpcUaDataType::Double),
        ("float64", OpcUaDataType::Double),
        ("string", OpcUaDataType::String),
    ] {
        for candidate in [format!(" {text} "), text.to_ascii_uppercase()] {
            assert_eq!(
                parse_opcua_client_data_type(candidate.as_str()).expect("documented scalar alias"),
                expected,
                "{candidate}"
            );
        }
    }

    for (text, expected) in [
        ("read", OpcUaClientPointAccess::Read),
        ("write", OpcUaClientPointAccess::Write),
        ("read_write", OpcUaClientPointAccess::ReadWrite),
        ("readwrite", OpcUaClientPointAccess::ReadWrite),
        ("read-write", OpcUaClientPointAccess::ReadWrite),
    ] {
        for candidate in [format!(" {text} "), text.to_ascii_uppercase()] {
            assert_eq!(
                parse_opcua_client_access(Some(candidate.as_str()), None)
                    .expect("documented access alias"),
                expected,
                "{candidate}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn client_pki_operations_do_not_follow_symbolic_links() {
    use std::os::unix::fs::symlink;

    let _guard = OPCUA_CLIENT_PKI_ENV_LOCK
        .lock()
        .expect("OPC UA client PKI env lock");
    let unique = unique_pki_suffix();
    let root = std::env::temp_dir().join(format!("trust-opcua-pki-link-root-{unique}"));
    let trusted_external =
        std::env::temp_dir().join(format!("trust-opcua-pki-link-trusted-{unique}"));
    let rejected_external =
        std::env::temp_dir().join(format!("trust-opcua-pki-link-rejected-{unique}"));
    std::fs::create_dir_all(root.join("trusted")).expect("trusted root");
    std::fs::create_dir_all(root.join("rejected")).expect("rejected root");
    std::fs::create_dir_all(&trusted_external).expect("external trusted root");
    std::fs::create_dir_all(&rejected_external).expect("external rejected root");
    let trusted_cert = trusted_external.join("outside.der");
    let rejected_cert = rejected_external.join("outside.pem");
    std::fs::write(&trusted_cert, b"trusted outside").expect("external trusted cert");
    std::fs::write(&rejected_cert, b"rejected outside").expect("external rejected cert");
    symlink(&trusted_external, root.join("trusted").join("escape"))
        .expect("trusted directory symlink");
    symlink(&rejected_external, root.join("rejected").join("escape"))
        .expect("rejected directory symlink");
    std::env::set_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &root);

    let listed = list_trusted_opcua_client_server_certificates().expect("list trusted");
    let cleared = clear_trusted_opcua_client_server_certificates().expect("clear trusted");
    let promoted = promote_rejected_opcua_client_server_certificates().expect("promote rejected");
    let trusted_preserved = trusted_cert.is_file();
    let rejected_preserved = rejected_cert.is_file();

    std::env::remove_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR");
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(trusted_external);
    let _ = std::fs::remove_dir_all(rejected_external);

    assert!(
        listed.is_empty(),
        "trusted listing escaped through a symbolic-link directory"
    );
    assert_eq!(cleared, 0);
    assert_eq!(promoted, 0);
    assert!(trusted_preserved, "clear removed an external certificate");
    assert!(
        rejected_preserved,
        "promotion removed an external rejected certificate"
    );
}

#[cfg(unix)]
#[test]
fn client_pki_root_links_are_not_traversed() {
    use std::os::unix::fs::symlink;

    let _guard = OPCUA_CLIENT_PKI_ENV_LOCK
        .lock()
        .expect("OPC UA client PKI env lock");
    let unique = unique_pki_suffix();

    let root_link_case = std::env::temp_dir().join(format!("trust-opcua-pki-root-link-{unique}"));
    let external_trusted =
        std::env::temp_dir().join(format!("trust-opcua-pki-root-external-{unique}"));
    std::fs::create_dir_all(&root_link_case).expect("root-link fixture");
    std::fs::create_dir_all(root_link_case.join("rejected")).expect("rejected fixture");
    std::fs::create_dir_all(&external_trusted).expect("external trusted fixture");
    let external_listed = external_trusted.join("outside.der");
    std::fs::write(&external_listed, b"outside").expect("external listed cert");
    let rejected = root_link_case.join("rejected").join("candidate.der");
    std::fs::write(&rejected, b"candidate").expect("rejected candidate");
    symlink(&external_trusted, root_link_case.join("trusted")).expect("trusted root symlink");
    std::env::set_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &root_link_case);
    let listed_via_root_link =
        list_trusted_opcua_client_server_certificates().expect("list linked trusted root");
    let promotion_via_root_link = promote_rejected_opcua_client_server_certificates();
    let rejected_preserved = rejected.is_file();
    let external_preserved =
        std::fs::read(&external_listed).expect("external cert after rejected promotion");
    std::env::remove_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR");
    let _ = std::fs::remove_dir_all(&root_link_case);
    let _ = std::fs::remove_dir_all(&external_trusted);

    assert!(
        listed_via_root_link.is_empty(),
        "listing followed a trusted root symbolic link"
    );
    assert!(
        promotion_via_root_link.is_err(),
        "promotion followed a trusted root symbolic link"
    );
    assert!(
        rejected_preserved,
        "failed root-link promotion removed the rejected source"
    );
    assert_eq!(
        external_preserved, b"outside",
        "promotion through a linked root modified an external certificate"
    );
}

#[cfg(unix)]
#[test]
fn client_pki_promotion_target_link_is_rejected() {
    use std::os::unix::fs::symlink;

    let _guard = OPCUA_CLIENT_PKI_ENV_LOCK
        .lock()
        .expect("OPC UA client PKI env lock");
    let unique = unique_pki_suffix();
    let target_link_case =
        std::env::temp_dir().join(format!("trust-opcua-pki-target-link-{unique}"));
    let external_target =
        std::env::temp_dir().join(format!("trust-opcua-pki-target-external-{unique}.pem"));
    std::fs::create_dir_all(target_link_case.join("trusted")).expect("trusted target root");
    std::fs::create_dir_all(target_link_case.join("rejected")).expect("rejected target root");
    let rejected = target_link_case.join("rejected").join("server.pem");
    std::fs::write(&rejected, b"candidate").expect("rejected candidate");
    std::fs::write(&external_target, b"external original").expect("external target");
    symlink(
        &external_target,
        target_link_case.join("trusted").join("server.pem"),
    )
    .expect("promotion target symlink");
    std::env::set_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &target_link_case);
    let promotion = promote_rejected_opcua_client_server_certificates();
    let external_after = std::fs::read(&external_target).expect("external target after promotion");
    let rejected_after = rejected.exists();
    std::env::remove_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR");
    let _ = std::fs::remove_dir_all(&target_link_case);
    let _ = std::fs::remove_file(&external_target);

    assert!(
        promotion.is_err(),
        "promotion followed an existing destination symbolic link"
    );
    assert_eq!(
        external_after, b"external original",
        "promotion overwrote a file outside the PKI root"
    );
    assert!(
        rejected_after,
        "failed promotion removed the rejected source certificate"
    );
}

#[test]
fn one_shot_service_vectors_require_exact_cardinality() {
    for (operation, expected, actual) in [
        ("read", 2, 1),
        ("read", 1, 2),
        ("write", 3, 2),
        ("write", 2, 3),
    ] {
        let error = validate_opcua_result_cardinality(operation, expected, actual)
            .expect_err("mismatched service vector must fail");
        let detail = error.to_string();
        assert!(
            detail.contains(operation)
                && detail.contains(expected.to_string().as_str())
                && detail.contains(actual.to_string().as_str()),
            "unexpected cardinality diagnostic: {detail}"
        );
    }
    validate_opcua_result_cardinality("read", 2, 2).expect("exact read vector");
    validate_opcua_result_cardinality("write", 2, 2).expect("exact write vector");
}

#[test]
fn client_error_classification_is_stable() {
    let cases = [
        (
            "BadCertificateUntrusted",
            OpcUaClientErrorCode::CertUntrusted,
        ),
        (
            "BadIdentityTokenRejected",
            OpcUaClientErrorCode::AuthRequired,
        ),
        (
            "no matching OPC UA endpoint for basic256sha256 / sign",
            OpcUaClientErrorCode::UnsupportedSecurityProfile,
        ),
        (
            "connection refused",
            OpcUaClientErrorCode::EndpointUnreachable,
        ),
    ];
    for (message, expected) in cases {
        let error = RuntimeError::ControlError(message.to_string().into());
        assert_eq!(classify_opcua_client_error(&error), expected, "{message}");
    }

    for message in ["BadUserAccessDenied", "BadNotReadable", "BadNodeIdUnknown"] {
        let error = RuntimeError::ControlError(message.to_string().into());
        assert_eq!(
            classify_opcua_client_browse_error(&error),
            OpcUaClientErrorCode::BrowseDenied,
            "{message}"
        );
    }
}
