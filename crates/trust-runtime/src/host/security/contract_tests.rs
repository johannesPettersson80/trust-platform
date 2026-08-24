use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

const CERT_PEM: &[u8] = include_bytes!("../../../tests/fixtures/tls/server-cert.pem");
const KEY_PEM: &[u8] = include_bytes!("../../../tests/fixtures/tls/server-key.pem");

static NEXT_DIR: AtomicU64 = AtomicU64::new(1);

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "trust-security-contract-{}-{label}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn config(
    mode: TlsMode,
    cert_path: Option<&str>,
    key_path: Option<&str>,
    ca_path: Option<&str>,
) -> TlsConfig {
    TlsConfig {
        mode,
        cert_path: cert_path.map(PathBuf::from),
        key_path: key_path.map(PathBuf::from),
        ca_path: ca_path.map(PathBuf::from),
        require_remote: false,
    }
}

fn materials(cert: &[u8], key: &[u8], ca: &[u8]) -> TlsMaterials {
    TlsMaterials {
        cert_path: PathBuf::from("cert.pem"),
        key_path: PathBuf::from("key.pem"),
        ca_path: Some(PathBuf::from("ca.pem")),
        certificate_pem: cert.to_vec(),
        private_key_pem: key.to_vec(),
        ca_pem: ca.to_vec(),
    }
}

fn error_text(error: RuntimeError) -> String {
    error.to_string()
}

#[test]
fn access_role_parse_is_trimmed_ascii_case_insensitive_and_closed() {
    for (text, expected) in [
        ("viewer", AccessRole::Viewer),
        (" VIEWER ", AccessRole::Viewer),
        ("\tOperator\n", AccessRole::Operator),
        ("EnGiNeEr", AccessRole::Engineer),
        ("admin", AccessRole::Admin),
    ] {
        assert_eq!(AccessRole::parse(text), Some(expected), "text={text:?}");
    }
    for text in ["", "view", "administrator", "engineer role", "viewer\0"] {
        assert_eq!(AccessRole::parse(text), None, "text={text:?}");
    }
}

#[test]
fn access_role_display_and_serialization_use_lowercase_wire_tokens() {
    for (role, expected) in [
        (AccessRole::Viewer, "viewer"),
        (AccessRole::Operator, "operator"),
        (AccessRole::Engineer, "engineer"),
        (AccessRole::Admin, "admin"),
    ] {
        assert_eq!(role.as_str(), expected);
        assert_eq!(
            serde_json::to_value(role).expect("serialize role"),
            serde_json::json!(expected)
        );
        assert_eq!(
            serde_json::from_value::<AccessRole>(serde_json::json!(expected))
                .expect("deserialize role"),
            role
        );
    }
}

#[test]
fn access_role_allows_exact_total_order() {
    let roles = [
        AccessRole::Viewer,
        AccessRole::Operator,
        AccessRole::Engineer,
        AccessRole::Admin,
    ];
    for (actual_index, actual) in roles.into_iter().enumerate() {
        for (required_index, required) in roles.into_iter().enumerate() {
            assert_eq!(
                actual.allows(required),
                actual_index >= required_index,
                "actual={actual:?}, required={required:?}"
            );
        }
    }
}

#[test]
fn constant_time_secret_equality_requires_exact_bytes_and_length() {
    for (expected, provided, equal) in [
        ("", "", true),
        ("a", "a", true),
        ("secret", "secret", true),
        ("secret", "Secret", false),
        ("secret", "secret ", false),
        ("secret", "secre", false),
        ("secret", "secret-extra", false),
        ("åäö", "åäö", true),
        ("åäö", "åäo", false),
        ("a\0b", "a\0b", true),
        ("a\0b", "a\0c", false),
    ] {
        assert_eq!(
            constant_time_eq(expected, provided),
            equal,
            "expected={expected:?}, provided={provided:?}"
        );
    }
}

#[test]
fn disabled_tls_loading_is_inert_without_paths_or_project_root() {
    let config = config(TlsMode::Disabled, None, None, None);
    assert!(load_tls_materials(&config, None)
        .expect("disabled TLS")
        .is_none());
}

#[test]
fn enabled_tls_requires_certificate_path_before_file_access() {
    let config = config(TlsMode::SelfManaged, None, Some("key.pem"), None);
    let error =
        load_tls_materials(&config, Some(Path::new("/project"))).expect_err("missing certificate");
    assert!(error_text(error).contains("missing tls cert_path"));
}

#[test]
fn enabled_tls_requires_private_key_path_before_file_access() {
    let config = config(TlsMode::SelfManaged, Some("cert.pem"), None, None);
    let error = load_tls_materials(&config, Some(Path::new("/project"))).expect_err("missing key");
    assert!(error_text(error).contains("missing tls key_path"));
}

#[test]
fn relative_tls_path_requires_project_root() {
    let error = resolve_tls_path(Path::new("certs/server.pem"), None)
        .expect_err("relative path without root");
    assert!(error_text(error).contains("relative tls path requires project root"));
}

#[test]
fn relative_tls_path_resolves_lexically_against_project_root() {
    assert_eq!(
        resolve_tls_path(Path::new("certs/server.pem"), Some(Path::new("/project")))
            .expect("resolve relative"),
        PathBuf::from("/project/certs/server.pem")
    );
}

#[test]
fn absolute_tls_path_is_preserved_without_project_root() {
    let absolute = temp_dir("absolute").join("server.pem");
    assert_eq!(
        resolve_tls_path(&absolute, None).expect("resolve absolute"),
        absolute
    );
}

#[test]
fn tls_material_loading_reads_relative_certificate_key_and_ca() {
    let root = temp_dir("relative");
    std::fs::write(root.join("cert.pem"), CERT_PEM).expect("write cert");
    std::fs::write(root.join("key.pem"), KEY_PEM).expect("write key");
    std::fs::write(root.join("ca.pem"), CERT_PEM).expect("write ca");
    let config = config(
        TlsMode::Provisioned,
        Some("cert.pem"),
        Some("key.pem"),
        Some("ca.pem"),
    );

    let loaded = load_tls_materials(&config, Some(&root))
        .expect("load materials")
        .expect("enabled materials");
    assert_eq!(loaded.cert_path, root.join("cert.pem"));
    assert_eq!(loaded.key_path, root.join("key.pem"));
    assert_eq!(
        loaded.ca_path.as_deref(),
        Some(root.join("ca.pem").as_path())
    );
    assert_eq!(loaded.certificate_pem, CERT_PEM);
    assert_eq!(loaded.private_key_pem, KEY_PEM);
    assert_eq!(loaded.ca_pem, CERT_PEM);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn omitted_ca_uses_owned_certificate_bytes_as_trust_root() {
    let root = temp_dir("default-ca");
    std::fs::write(root.join("cert.pem"), CERT_PEM).expect("write cert");
    std::fs::write(root.join("key.pem"), KEY_PEM).expect("write key");
    let config = config(
        TlsMode::SelfManaged,
        Some("cert.pem"),
        Some("key.pem"),
        None,
    );

    let mut loaded = load_tls_materials(&config, Some(&root))
        .expect("load materials")
        .expect("enabled materials");
    assert_eq!(loaded.ca_path, None);
    assert_eq!(loaded.ca_pem, loaded.certificate_pem);
    loaded.certificate_pem[0] ^= 1;
    assert_ne!(loaded.ca_pem, loaded.certificate_pem);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn certificate_read_failure_identifies_role_and_resolved_path() {
    let root = temp_dir("missing-cert");
    std::fs::write(root.join("key.pem"), KEY_PEM).expect("write key");
    let config = config(
        TlsMode::SelfManaged,
        Some("missing-cert.pem"),
        Some("key.pem"),
        None,
    );

    let text =
        error_text(load_tls_materials(&config, Some(&root)).expect_err("missing certificate"));
    assert!(text.contains("read tls cert"));
    assert!(text.contains(root.join("missing-cert.pem").to_string_lossy().as_ref()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn private_key_read_failure_identifies_role_and_resolved_path() {
    let root = temp_dir("missing-key");
    std::fs::write(root.join("cert.pem"), CERT_PEM).expect("write cert");
    let config = config(
        TlsMode::SelfManaged,
        Some("cert.pem"),
        Some("missing-key.pem"),
        None,
    );

    let text = error_text(load_tls_materials(&config, Some(&root)).expect_err("missing key"));
    assert!(text.contains("read tls key"));
    assert!(text.contains(root.join("missing-key.pem").to_string_lossy().as_ref()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ca_read_failure_identifies_role_and_resolved_path() {
    let root = temp_dir("missing-ca");
    std::fs::write(root.join("cert.pem"), CERT_PEM).expect("write cert");
    std::fs::write(root.join("key.pem"), KEY_PEM).expect("write key");
    let config = config(
        TlsMode::Provisioned,
        Some("cert.pem"),
        Some("key.pem"),
        Some("missing-ca.pem"),
    );

    let text = error_text(load_tls_materials(&config, Some(&root)).expect_err("missing ca"));
    assert!(text.contains("read tls ca"));
    assert!(text.contains(root.join("missing-ca.pem").to_string_lossy().as_ref()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tiny_http_projection_is_an_owned_cert_and_key_copy() {
    let mut materials = materials(CERT_PEM, KEY_PEM, CERT_PEM);
    let projected = materials.tiny_http_ssl_config();
    materials.certificate_pem[0] ^= 1;
    materials.private_key_pem[0] ^= 1;

    assert_eq!(projected.certificate, CERT_PEM);
    assert_eq!(projected.private_key, KEY_PEM);
}

#[test]
fn certificate_parser_accepts_one_and_multiple_certificates() {
    assert_eq!(
        parse_pem_certs(CERT_PEM, "test certificate")
            .expect("one certificate")
            .len(),
        1
    );
    let chain = [CERT_PEM, CERT_PEM].concat();
    assert_eq!(
        parse_pem_certs(&chain, "test chain")
            .expect("certificate chain")
            .len(),
        2
    );
}

#[test]
fn certificate_parser_rejects_empty_noncertificate_and_truncated_pem() {
    for pem in [
        b"".as_slice(),
        b"plain text".as_slice(),
        b"-----BEGIN CERTIFICATE-----\nAAAA\n".as_slice(),
    ] {
        let text = error_text(parse_pem_certs(pem, "owned label").expect_err("invalid cert"));
        assert!(text.contains("parse owned label"));
    }
}

#[test]
fn private_key_parser_accepts_fixture_and_rejects_invalid_material() {
    assert!(parse_pem_key(KEY_PEM, "test key").is_ok());
    for pem in [
        b"".as_slice(),
        b"plain text".as_slice(),
        b"-----BEGIN PRIVATE KEY-----\nAAAA\n".as_slice(),
    ] {
        let text = error_text(parse_pem_key(pem, "owned key").expect_err("invalid key"));
        assert!(text.contains("parse owned key"));
    }
}

#[test]
fn rustls_server_config_accepts_matching_fixture_material() {
    let config =
        rustls_server_config(&materials(CERT_PEM, KEY_PEM, CERT_PEM)).expect("server config");
    assert_eq!(Arc::strong_count(&config), 1);
}

#[test]
fn rustls_server_config_rejects_empty_certificate_and_invalid_key() {
    let empty_cert =
        rustls_server_config(&materials(b"", KEY_PEM, CERT_PEM)).expect_err("empty cert");
    assert!(error_text(empty_cert).contains("no certificates found"));

    let invalid_key =
        rustls_server_config(&materials(CERT_PEM, b"invalid", CERT_PEM)).expect_err("invalid key");
    assert!(error_text(invalid_key).contains("parse tls private key"));
}

#[test]
fn rustls_client_config_accepts_fixture_trust_root() {
    let config =
        rustls_client_config(&materials(CERT_PEM, KEY_PEM, CERT_PEM)).expect("client config");
    assert_eq!(Arc::strong_count(&config), 1);
}

#[test]
fn rustls_client_config_rejects_empty_and_malformed_ca() {
    let empty = rustls_client_config(&materials(CERT_PEM, KEY_PEM, b"")).expect_err("empty ca");
    assert!(error_text(empty).contains("no certificates found"));

    let malformed = rustls_client_config(&materials(CERT_PEM, KEY_PEM, b"plain text"))
        .expect_err("malformed ca");
    assert!(error_text(malformed).contains("parse tls ca certificate"));
}

#[test]
fn rustls_provider_installation_is_repeatable() {
    ensure_rustls_crypto_provider().expect("first provider check");
    ensure_rustls_crypto_provider().expect("second provider check");
}
