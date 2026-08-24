fn source(id: u32, path: impl Into<PathBuf>, text: &str) -> SourceFile {
    SourceFile {
        id,
        path: path.into(),
        text: text.into(),
    }
}

#[test]
fn control_endpoint_accepts_ipv4_loopback_port_boundaries() {
    for (text, expected) in [
        ("tcp://127.0.0.1:0", "127.0.0.1:0"),
        ("tcp://127.0.0.1:65535", "127.0.0.1:65535"),
    ] {
        match ControlEndpoint::parse(text).expect("TCP endpoint") {
            ControlEndpoint::Tcp(address) => assert_eq!(address.to_string(), expected),
            #[cfg(unix)]
            ControlEndpoint::Unix(path) => panic!("unexpected Unix endpoint: {path:?}"),
        }
    }
}

#[test]
fn control_endpoint_accepts_bracketed_ipv6_loopback() {
    match ControlEndpoint::parse("tcp://[::1]:3210").expect("IPv6 loopback") {
        ControlEndpoint::Tcp(address) => {
            assert!(address.ip().is_loopback());
            assert_eq!(address.port(), 3210);
        }
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => panic!("unexpected Unix endpoint: {path:?}"),
    }
}

#[test]
fn control_endpoint_rejects_nonloopback_numeric_addresses() {
    for text in [
        "tcp://0.0.0.0:9000",
        "tcp://192.0.2.1:9000",
        "tcp://10.0.0.1:9000",
        "tcp://[::]:9000",
        "tcp://[2001:db8::1]:9000",
    ] {
        let message = ControlEndpoint::parse(text)
            .expect_err("non-loopback endpoint")
            .to_string();
        assert!(
            message.contains("tcp endpoint must be loopback"),
            "text={text:?}, message={message}"
        );
    }
}

#[test]
fn control_endpoint_rejects_hostname_and_malformed_tcp_shapes() {
    for text in [
        "tcp://localhost:9000",
        "tcp://127.0.0.1",
        "tcp://127.0.0.1:",
        "tcp://:9000",
        "tcp://127.0.0.1:65536",
        "tcp://127.0.0.1:9000/extra",
        "tcp://[::1]",
        "tcp://::1:9000",
    ] {
        let message = ControlEndpoint::parse(text)
            .expect_err("malformed TCP endpoint")
            .to_string();
        assert!(
            message.contains("invalid tcp endpoint"),
            "text={text:?}, message={message}"
        );
    }
}

#[test]
fn control_endpoint_grammar_is_case_and_whitespace_sensitive() {
    for text in [
        " TCP://127.0.0.1:9000",
        "TCP://127.0.0.1:9000",
        "tcp://127.0.0.1:9000 ",
        "\ttcp://127.0.0.1:9000",
        "",
        "127.0.0.1:9000",
    ] {
        assert!(
            ControlEndpoint::parse(text)
                .expect_err("unsupported endpoint")
                .to_string()
                .contains("unsupported endpoint")
                || text.starts_with("tcp://"),
            "text={text:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn unix_control_endpoint_preserves_absolute_and_relative_paths() {
    for (text, expected) in [
        ("unix:///run/trust/control.sock", "/run/trust/control.sock"),
        ("unix://run/control.sock", "run/control.sock"),
    ] {
        match ControlEndpoint::parse(text).expect("Unix endpoint") {
            ControlEndpoint::Unix(path) => assert_eq!(path, PathBuf::from(expected)),
            ControlEndpoint::Tcp(address) => panic!("unexpected TCP endpoint: {address}"),
        }
    }
}

#[cfg(unix)]
#[test]
fn unix_control_endpoint_rejects_empty_path() {
    let message = ControlEndpoint::parse("unix://")
        .expect_err("empty Unix path")
        .to_string();
    assert!(message.contains("unix endpoint path must not be empty"));
}

#[cfg(unix)]
#[test]
fn unix_control_endpoint_grammar_is_case_and_whitespace_sensitive() {
    for text in [
        "UNIX:///run/control.sock",
        " unix:///run/control.sock",
        "unix:///run/control.sock ",
    ] {
        assert!(
            ControlEndpoint::parse(text).is_err(),
            "text={text:?} unexpectedly accepted"
        );
    }
}

#[test]
fn source_registry_preserves_registration_order_and_exact_records() {
    let registry = SourceRegistry::new(vec![
        source(2, "src/second.st", "SECOND"),
        source(1, "src/first.st", "FIRST"),
    ]);

    assert!(!registry.is_empty());
    assert_eq!(registry.files().len(), 2);
    assert_eq!(registry.files()[0].id, 2);
    assert_eq!(registry.files()[0].path, PathBuf::from("src/second.st"));
    assert_eq!(registry.files()[0].text, "SECOND");
    assert_eq!(registry.files()[1].id, 1);
}

#[test]
fn empty_source_registry_has_no_paths_or_text() {
    let registry = SourceRegistry::default();
    assert!(registry.is_empty());
    assert!(registry.files().is_empty());
    assert_eq!(registry.file_id_for_path(Path::new("main.st")), None);
    assert_eq!(registry.source_text(1), None);
}

#[test]
fn source_text_lookup_is_exact_and_uses_first_duplicate_id() {
    let registry = SourceRegistry::new(vec![
        source(7, "src/first.st", "FIRST"),
        source(7, "src/second.st", "SECOND"),
    ]);

    assert_eq!(registry.source_text(7), Some("FIRST"));
    assert_eq!(registry.source_text(8), None);
}

#[test]
fn source_lookup_prefers_exact_stored_path() {
    let registry = SourceRegistry::new(vec![
        source(1, "other/main.st", "OTHER"),
        source(2, "src/main.st", "EXACT"),
    ]);

    assert_eq!(
        registry.file_id_for_path(Path::new("src/main.st")),
        Some(2)
    );
}

#[test]
fn project_relative_source_lookup_checks_root_join() {
    let root = Path::new("/project");
    let registry = SourceRegistry::new(vec![source(1, "/project/generated/main.st", "MAIN")]);

    assert_eq!(
        registry.file_id_for_path_in_project(
            Path::new("generated/main.st"),
            Some(root)
        ),
        Some(1)
    );
}

#[test]
fn single_component_source_lookup_checks_project_src_directory() {
    let root = Path::new("/project");
    let registry = SourceRegistry::new(vec![source(1, "/project/src/main.st", "MAIN")]);

    assert_eq!(
        registry.file_id_for_path_in_project(Path::new("main.st"), Some(root)),
        Some(1)
    );
}

#[test]
fn multi_component_source_lookup_does_not_invent_project_src_prefix() {
    let root = Path::new("/project");
    let registry = SourceRegistry::new(vec![
        source(1, "/project/src/generated/main.st", "MAIN"),
        source(2, "/other/generated/main.st", "OTHER"),
    ]);

    assert_eq!(
        registry.file_id_for_path_in_project(
            Path::new("generated/main.st"),
            Some(root)
        ),
        None
    );
}

#[test]
fn unique_relative_suffix_match_resolves_nested_source() {
    let registry = SourceRegistry::new(vec![
        source(1, "/project/src/line/main.st", "MAIN"),
        source(2, "/project/src/other.st", "OTHER"),
    ]);

    assert_eq!(
        registry.file_id_for_path(Path::new("line/main.st")),
        Some(1)
    );
    assert_eq!(registry.file_id_for_path(Path::new("other.st")), Some(2));
}

#[test]
fn ambiguous_relative_suffix_match_fails_closed() {
    let registry = SourceRegistry::new(vec![
        source(1, "/project/a/main.st", "A"),
        source(2, "/project/b/main.st", "B"),
    ]);

    assert_eq!(registry.file_id_for_path(Path::new("main.st")), None);
}

#[test]
fn unmatched_absolute_path_does_not_use_suffix_fallback() {
    let registry = SourceRegistry::new(vec![source(
        1,
        "/project/src/main.st",
        "MAIN",
    )]);

    assert_eq!(
        registry.file_id_for_path(Path::new("/different/src/main.st")),
        None
    );
}

#[test]
fn suffix_match_requires_complete_path_component_boundaries() {
    let registry = SourceRegistry::new(vec![source(
        1,
        "/project/src/not-main.st",
        "OTHER",
    )]);

    assert_eq!(registry.file_id_for_path(Path::new("main.st")), None);
}

#[test]
fn unique_suffix_match_returns_first_only_when_no_second_match_exists() {
    let registry = SourceRegistry::new(vec![
        source(1, "/project/a/main.st", "A"),
        source(2, "/project/b/other.st", "B"),
    ]);
    assert_eq!(
        registry
            .unique_suffix_match(Path::new("main.st"))
            .map(|file| file.id),
        Some(1)
    );

    let ambiguous = SourceRegistry::new(vec![
        source(1, "/project/a/main.st", "A"),
        source(2, "/project/b/main.st", "B"),
    ]);
    assert!(ambiguous
        .unique_suffix_match(Path::new("main.st"))
        .is_none());
}

#[test]
fn canonical_path_equality_requires_existing_left_path_and_exact_target() {
    let root = temp_dir("control-source-canonical");
    let first = root.join("first.st");
    let second = root.join("second.st");
    write_file(&first, "FIRST");
    write_file(&second, "SECOND");
    let first_canonical = std::fs::canonicalize(&first).expect("canonical first");
    let second_canonical = std::fs::canonicalize(&second).expect("canonical second");

    assert!(canonical_path_eq(&first, &first_canonical));
    assert!(!canonical_path_eq(&first, &second_canonical));
    assert!(!canonical_path_eq(
        &root.join("missing.st"),
        &first_canonical
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn source_lookup_matches_existing_symlink_candidate_to_registered_canonical_path() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("control-source-symlink");
    let real = root.join("real.st");
    let alias = root.join("alias.st");
    write_file(&real, "MAIN");
    symlink(&real, &alias).expect("create symlink");
    let registry = SourceRegistry::new(vec![source(7, real.clone(), "MAIN")]);

    assert_eq!(registry.file_id_for_path(&alias), Some(7));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn control_server_accessors_preserve_endpoint_and_shared_state_identity() {
    let state = Arc::new(hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    ));
    let endpoint = ControlEndpoint::parse("tcp://127.0.0.1:0").expect("endpoint");
    let server = ControlServer {
        endpoint,
        state: Arc::clone(&state),
    };

    assert!(matches!(server.endpoint(), ControlEndpoint::Tcp(address) if address.port() == 0));
    assert!(Arc::ptr_eq(&server.state(), &state));
}

#[test]
fn hmi_descriptor_startup_timeout_uses_bounded_test_contract() {
    assert_eq!(
        hmi_descriptor_watch_startup_timeout(),
        Duration::from_secs(10)
    );
}
