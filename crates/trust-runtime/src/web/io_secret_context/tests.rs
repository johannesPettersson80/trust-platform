use super::*;

fn stored() -> toml::Value {
    toml::from_str("broker='127.0.0.1:1883'\nusername='plant'\npassword='private-value'").unwrap()
}

fn projected(stored: &toml::Value) -> toml::Value {
    let mut value = stored.clone();
    redact(&mut value);
    value
}

#[test]
fn mqtt_retention_binds_destination_identity_and_policy() {
    let stored = stored();
    for (key, value) in [
        ("broker", toml::Value::String("localhost:1883".into())),
        ("broker", toml::Value::String("127.0.0.1:1884".into())),
        ("username", toml::Value::String("other-user".into())),
        ("client_id", toml::Value::String("other-client".into())),
        ("allow_insecure_remote", toml::Value::Boolean(true)),
    ] {
        for driver in ["mqtt", "mqtt-tcp"] {
            let mut requested = projected(&stored);
            requested
                .as_table_mut()
                .unwrap()
                .insert(key.into(), value.clone());
            let error = check(driver, &requested, Some(&stored))
                .unwrap_err()
                .to_string();
            assert!(!error.contains("private-value"));
            requested
                .as_table_mut()
                .unwrap()
                .insert("password".into(), toml::Value::String("replacement".into()));
            check(driver, &requested, Some(&stored)).unwrap();
        }
    }
}

#[test]
fn mqtt_retention_allows_operational_edits_and_effective_defaults() {
    let stored = stored();
    let mut requested = projected(&stored);
    requested.as_table_mut().unwrap().insert(
        "broker".into(),
        toml::Value::String("mqtt://127.0.0.1:1883".into()),
    );
    requested
        .as_table_mut()
        .unwrap()
        .insert("tls".into(), toml::Value::Boolean(false));
    requested
        .as_table_mut()
        .unwrap()
        .insert("allow_insecure_remote".into(), toml::Value::Boolean(false));
    requested
        .as_table_mut()
        .unwrap()
        .insert("topic_in".into(), toml::Value::String("new-input".into()));
    requested
        .as_table_mut()
        .unwrap()
        .insert("topic_out".into(), toml::Value::String("new-output".into()));
    requested
        .as_table_mut()
        .unwrap()
        .insert("reconnect_ms".into(), toml::Value::Integer(750));
    requested
        .as_table_mut()
        .unwrap()
        .insert("keep_alive_s".into(), toml::Value::Integer(30));
    requested
        .as_table_mut()
        .unwrap()
        .insert("on_error".into(), toml::Value::String("warn".into()));
    check("mqtt", &requested, Some(&stored)).unwrap();
}

#[test]
fn mqtt_retention_binds_tls_material_paths_and_alpn() {
    let dir = TempDirectory::new();
    let ca = dir.0.join("ca.pem");
    let other = dir.0.join("other.pem");
    std::fs::write(&ca, b"fixture CA material").unwrap();
    std::fs::write(&other, b"fixture CA material").unwrap();
    let mut stored = stored();
    stored
        .as_table_mut()
        .unwrap()
        .insert("tls".into(), toml::Value::Boolean(true));
    stored.as_table_mut().unwrap().insert(
        "tls_ca_path".into(),
        toml::Value::String(ca.to_string_lossy().into_owned()),
    );
    let requested = projected(&stored);
    check("mqtt", &requested, Some(&stored)).unwrap();
    let mut changed_path = requested.clone();
    changed_path.as_table_mut().unwrap().insert(
        "tls_ca_path".into(),
        toml::Value::String(other.to_string_lossy().into_owned()),
    );
    assert!(check("mqtt", &changed_path, Some(&stored)).is_err());
    let mut alpn = requested.clone();
    alpn.as_table_mut().unwrap().insert(
        "tls_alpn".into(),
        toml::Value::Array(vec![toml::Value::String("mqtt".into())]),
    );
    assert!(check("mqtt", &alpn, Some(&stored)).is_err());
    let mut downgraded = requested.clone();
    downgraded.as_table_mut().unwrap().remove("tls_ca_path");
    downgraded
        .as_table_mut()
        .unwrap()
        .insert("tls".into(), toml::Value::Boolean(false));
    assert!(check("mqtt", &downgraded, Some(&stored)).is_err());
    let mut mtls = requested;
    mtls.as_table_mut().unwrap().insert(
        "tls_client_cert_path".into(),
        toml::Value::String(ca.to_string_lossy().into_owned()),
    );
    mtls.as_table_mut().unwrap().insert(
        "tls_client_key_path".into(),
        toml::Value::String(other.to_string_lossy().into_owned()),
    );
    assert!(check("mqtt", &mtls, Some(&stored)).is_err());
}

#[test]
fn unknown_driver_requires_the_complete_original_projection() {
    let stored: toml::Value = toml::from_str("broker='original'\npassword='private-value'\n[credentials]\nendpoint='original'\ntoken='other-private-value'").unwrap();
    let projection = projected(&stored);
    check("custom", &projection, Some(&stored)).unwrap();
    let mut modified = projection.clone();
    modified
        .as_table_mut()
        .unwrap()
        .insert("broker".into(), toml::Value::String("other".into()));
    assert!(check("custom", &modified, Some(&stored)).is_err());
    let mut modified = projection;
    modified.as_table_mut().unwrap().insert(
        "credentials".into(),
        toml::from_str("endpoint='other'\ntoken='replacement'").unwrap(),
    );
    assert!(check("custom", &modified, Some(&stored)).is_err());
    modified
        .as_table_mut()
        .unwrap()
        .insert("password".into(), toml::Value::String("replacement".into()));
    check("custom", &modified, Some(&stored)).unwrap();
}

struct TempDirectory(std::path::PathBuf);

impl TempDirectory {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        loop {
            let next = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("trust-io-context-{}-{next}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create fixture: {error}"),
            }
        }
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
