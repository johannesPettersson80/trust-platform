use std::sync::atomic::{AtomicU64, Ordering};

static REGISTRY_CONTRACT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

fn registry_contract_temp_dir(label: &str) -> PathBuf {
    for _ in 0..64 {
        let sequence = REGISTRY_CONTRACT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-registry-contract-{label}-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&root) {
            Ok(()) => return root,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("create {}: {error}", root.display()),
        }
    }
    panic!("failed to allocate registry contract directory for {label}");
}

fn registry_contract_bundle(root: &Path, resource_name: &str) {
    fs::create_dir_all(root.join("src")).expect("create bundle source");
    fs::write(
        root.join("runtime.toml"),
        format!(
            r#"[bundle]
version = 1

[resource]
name = "{resource_name}"
cycle_interval_ms = 100

[runtime.control]
endpoint = "tcp://127.0.0.1:0"
auth_token = "test-token"

[runtime.web]
enabled = false
listen = "127.0.0.1:0"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#
        ),
    )
    .expect("write runtime configuration");
    fs::write(
        root.join("io.toml"),
        r#"[io]
driver = "simulated"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
"#,
    )
    .expect("write I/O configuration");
    fs::write(root.join("program.stbc"), b"registry-contract-bytecode")
        .expect("write bytecode payload");
    fs::write(root.join("src/main.st"), b"PROGRAM Main\nEND_PROGRAM\n")
        .expect("write source payload");
}

fn registry_contract_init(root: &Path) {
    init_registry(root, RegistryVisibility::Public, None).expect("initialize public registry");
}

fn registry_contract_publish(
    registry_root: &Path,
    bundle_root: &Path,
    package_name: &str,
    version: &str,
) -> anyhow::Result<PublishReport> {
    publish_package(PublishRequest {
        registry_root: registry_root.to_path_buf(),
        bundle_root: bundle_root.to_path_buf(),
        package_name: Some(package_name.to_string()),
        version: version.to_string(),
        token: None,
    })
}

fn registry_contract_read(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn registry_contract_metadata(package_root: &Path) -> PackageMetadata {
    serde_json::from_slice(&registry_contract_read(
        &package_root.join(PACKAGE_METADATA_FILE),
    ))
    .expect("parse package metadata")
}

fn registry_contract_index(registry_root: &Path) -> RegistryIndex {
    serde_json::from_slice(&registry_contract_read(&registry_index_path(registry_root)))
        .expect("parse registry index")
}

fn registry_contract_remove(root: PathBuf) {
    let _ = fs::remove_dir_all(root);
}

#[test]
fn registry_initialization_creates_versioned_empty_layout() {
    let registry = registry_contract_temp_dir("init-layout");
    registry_contract_init(&registry);

    let settings = load_registry_settings(&registry).expect("load settings");
    let index = registry_contract_index(&registry);
    assert_eq!(settings.version, REGISTRY_SCHEMA_VERSION);
    assert_eq!(settings.visibility, RegistryVisibility::Public);
    assert_eq!(index.schema_version, REGISTRY_SCHEMA_VERSION);
    assert!(index.packages.is_empty());
    assert!(registry_packages_path(&registry).is_dir());

    registry_contract_remove(registry);
}

#[test]
fn registry_private_initialization_rejects_blank_token_before_usable_state() {
    for token in [None, Some(String::new()), Some(" \t ".to_string())] {
        let registry = registry_contract_temp_dir("private-blank-token");
        let error = init_registry(&registry, RegistryVisibility::Private, token)
            .expect_err("blank private token must fail");
        assert!(error.to_string().contains("non-empty auth_token"));
        assert!(!registry_config_path(&registry).exists());
        assert!(!registry_index_path(&registry).exists());
        registry_contract_remove(registry);
    }
}

#[test]
fn registry_existing_malformed_index_blocks_reinitialization_without_config_change() {
    for invalid_index in [
        b"{not-json".as_slice(),
        br#"{"schema_version":99,"generated_at_unix":0,"packages":[]}"#.as_slice(),
    ] {
        let registry = registry_contract_temp_dir("reinit-malformed-index");
        registry_contract_init(&registry);
        let config_before = registry_contract_read(&registry_config_path(&registry));
        fs::write(registry_index_path(&registry), invalid_index).expect("write invalid index");

        init_registry(
            &registry,
            RegistryVisibility::Private,
            Some("new-secret".to_string()),
        )
        .expect_err("invalid existing index must block reinitialization");

        assert_eq!(
            registry_contract_read(&registry_config_path(&registry)),
            config_before
        );
        assert_eq!(
            registry_contract_read(&registry_index_path(&registry)),
            invalid_index
        );
        registry_contract_remove(registry);
    }
}

#[test]
fn registry_reinitialization_preserves_existing_package_index() {
    let root = registry_contract_temp_dir("reinit-preserve");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    registry_contract_bundle(&bundle, "Cell");
    registry_contract_init(&registry);
    registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish package");
    let index_before = registry_contract_index(&registry);

    init_registry(
        &registry,
        RegistryVisibility::Private,
        Some("new-secret".to_string()),
    )
    .expect("reinitialize access configuration");

    let index_after = registry_contract_index(&registry);
    assert_eq!(index_after.packages.len(), 1);
    assert_eq!(index_after.packages[0].name, "cell");
    assert_eq!(
        index_after.packages[0].package_sha256,
        index_before.packages[0].package_sha256
    );
    assert!(package_root(&registry, "cell", "1.0.0").is_dir());
    registry_contract_remove(root);
}

#[test]
fn registry_private_operations_authenticate_before_package_state_access() {
    let root = registry_contract_temp_dir("private-preflight");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    registry_contract_bundle(&bundle, "PrivateCell");
    init_registry(
        &registry,
        RegistryVisibility::Private,
        Some("secret".to_string()),
    )
    .expect("initialize private registry");
    let index_before = registry_contract_read(&registry_index_path(&registry));

    for token in [None, Some(""), Some("wrong")] {
        let publish = publish_package(PublishRequest {
            registry_root: registry.clone(),
            bundle_root: bundle.clone(),
            package_name: Some("private-cell".to_string()),
            version: "1.0.0".to_string(),
            token: token.map(str::to_string),
        });
        assert!(publish
            .expect_err("unauthorized publish")
            .to_string()
            .contains("unauthorized"));
        assert!(!package_root(&registry, "private-cell", "1.0.0").exists());
        assert_eq!(
            registry_contract_read(&registry_index_path(&registry)),
            index_before
        );
    }

    registry_contract_remove(root);
}

#[test]
fn registry_identifiers_are_confined_single_path_segments() {
    let root = registry_contract_temp_dir("identity-segments");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    registry_contract_bundle(&bundle, "Cell");
    registry_contract_init(&registry);
    let index_before = registry_contract_read(&registry_index_path(&registry));

    for (name, version) in [
        (".", "1.0.0"),
        ("..", "1.0.0"),
        ("cell", "."),
        ("cell", ".."),
        ("../cell", "1.0.0"),
        ("cell", "../1.0.0"),
        ("/cell", "1.0.0"),
        ("cell", "/1.0.0"),
        ("cell/name", "1.0.0"),
        ("cell", "1.0/0"),
        ("cell\\name", "1.0.0"),
        ("cell", "1.0\\0"),
    ] {
        let error = registry_contract_publish(&registry, &bundle, name, version)
            .expect_err("non-segment identity must fail");
        assert!(
            error.to_string().contains("unsupported") || error.to_string().contains("path segment"),
            "unexpected identity error for {name}/{version}: {error}"
        );
    }

    assert_eq!(
        registry_contract_read(&registry_index_path(&registry)),
        index_before
    );
    assert!(registry_contract_index(&registry).packages.is_empty());
    registry_contract_remove(root);
}

#[test]
fn registry_lookup_operations_reject_unconfined_identities_before_io() {
    let registry = registry_contract_temp_dir("lookup-segments");
    registry_contract_init(&registry);

    for (name, version) in [
        (".", "1.0.0"),
        ("..", "1.0.0"),
        ("cell", "."),
        ("cell", ".."),
        ("../cell", "1.0.0"),
        ("cell", "../1.0.0"),
    ] {
        let verify_error = verify_package(VerifyRequest {
            registry_root: registry.clone(),
            name: name.to_string(),
            version: version.to_string(),
            token: None,
        })
        .expect_err("invalid verify identity");
        assert!(
            verify_error.to_string().contains("unsupported")
                || verify_error.to_string().contains("path segment")
        );

        let output = registry.join("download");
        let download_error = download_package(DownloadRequest {
            registry_root: registry.clone(),
            name: name.to_string(),
            version: version.to_string(),
            output_root: output.clone(),
            token: None,
            verify_before_install: true,
        })
        .expect_err("invalid download identity");
        assert!(
            download_error.to_string().contains("unsupported")
                || download_error.to_string().contains("path segment")
        );
        assert!(!output.exists());
    }

    registry_contract_remove(registry);
}

#[test]
fn registry_malformed_or_unsupported_index_blocks_publish_without_mutation() {
    for invalid_index in [
        b"{not-json".as_slice(),
        br#"{"schema_version":99,"generated_at_unix":0,"packages":[]}"#.as_slice(),
    ] {
        let root = registry_contract_temp_dir("publish-invalid-index");
        let registry = root.join("registry");
        let bundle = root.join("bundle");
        registry_contract_bundle(&bundle, "Cell");
        registry_contract_init(&registry);
        fs::write(registry_index_path(&registry), invalid_index).expect("write invalid index");

        registry_contract_publish(&registry, &bundle, "cell", "1.0.0")
            .expect_err("invalid index must fail publication");

        assert_eq!(
            registry_contract_read(&registry_index_path(&registry)),
            invalid_index
        );
        assert!(!package_root(&registry, "cell", "1.0.0").exists());
        registry_contract_remove(root);
    }
}

#[test]
fn registry_invalid_bundle_publish_leaves_index_and_identity_unchanged() {
    let root = registry_contract_temp_dir("invalid-bundle");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    fs::create_dir_all(&bundle).expect("create invalid bundle");
    registry_contract_init(&registry);
    let index_before = registry_contract_read(&registry_index_path(&registry));

    registry_contract_publish(&registry, &bundle, "cell", "1.0.0")
        .expect_err("invalid bundle must fail");

    assert_eq!(
        registry_contract_read(&registry_index_path(&registry)),
        index_before
    );
    assert!(!package_root(&registry, "cell", "1.0.0").exists());
    registry_contract_remove(root);
}

#[test]
fn registry_successful_publish_binds_sorted_complete_digest_metadata() {
    let root = registry_contract_temp_dir("metadata-digests");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    registry_contract_bundle(&bundle, "Cell");
    fs::create_dir_all(bundle.join("assets/z")).expect("create nested asset directory");
    fs::write(bundle.join("assets/z/last.bin"), b"last").expect("write last asset");
    fs::write(bundle.join("assets/first.bin"), b"first").expect("write first asset");
    registry_contract_init(&registry);

    let report =
        registry_contract_publish(&registry, &bundle, " cell ", " 1.0.0 ").expect("publish");
    let metadata = registry_contract_metadata(&report.package_root);
    let paths: Vec<&str> = metadata
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let mut sorted_paths = paths.clone();
    sorted_paths.sort_unstable();

    assert_eq!(metadata.name, "cell");
    assert_eq!(metadata.version, "1.0.0");
    assert_eq!(metadata.resource_name, "Cell");
    assert_eq!(paths, sorted_paths);
    assert_eq!(
        metadata.total_bytes,
        metadata.files.iter().map(|file| file.bytes).sum::<u64>()
    );
    assert!(metadata.files.iter().all(|file| {
        !file.path.starts_with('/')
            && !file.path.contains('\\')
            && file.sha256.len() == 64
            && file
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }));
    verify_package(VerifyRequest {
        registry_root: registry.clone(),
        name: "cell".to_string(),
        version: "1.0.0".to_string(),
        token: None,
    })
    .expect("verify published package");
    registry_contract_remove(root);
}

#[test]
fn registry_index_preserves_entries_and_orders_name_then_version() {
    let root = registry_contract_temp_dir("index-order");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    registry_contract_bundle(&bundle, "Cell");
    registry_contract_init(&registry);

    for (name, version) in [
        ("zeta", "2.0.0"),
        ("alpha", "10.0.0"),
        ("alpha", "2.0.0"),
        ("zeta", "1.0.0"),
    ] {
        registry_contract_publish(&registry, &bundle, name, version).expect("publish identity");
    }

    let identities: Vec<(String, String)> = registry_contract_index(&registry)
        .packages
        .into_iter()
        .map(|entry| (entry.name, entry.version))
        .collect();
    assert_eq!(
        identities,
        vec![
            ("alpha".to_string(), "10.0.0".to_string()),
            ("alpha".to_string(), "2.0.0".to_string()),
            ("zeta".to_string(), "1.0.0".to_string()),
            ("zeta".to_string(), "2.0.0".to_string()),
        ]
    );
    registry_contract_remove(root);
}

#[test]
fn registry_duplicate_identity_is_immutable() {
    let root = registry_contract_temp_dir("immutable");
    let registry = root.join("registry");
    let first_bundle = root.join("first");
    let second_bundle = root.join("second");
    registry_contract_bundle(&first_bundle, "Cell");
    registry_contract_bundle(&second_bundle, "Replacement");
    registry_contract_init(&registry);
    let first =
        registry_contract_publish(&registry, &first_bundle, "cell", "1.0.0").expect("publish");
    let index_before = registry_contract_read(&registry_index_path(&registry));
    let metadata_before = registry_contract_read(&first.metadata_path);
    let payload_before = registry_contract_read(&first.package_root.join("bundle/program.stbc"));

    registry_contract_publish(&registry, &second_bundle, "cell", "1.0.0")
        .expect_err("duplicate identity must fail");

    assert_eq!(
        registry_contract_read(&registry_index_path(&registry)),
        index_before
    );
    assert_eq!(
        registry_contract_read(&first.metadata_path),
        metadata_before
    );
    assert_eq!(
        registry_contract_read(&first.package_root.join("bundle/program.stbc")),
        payload_before
    );
    registry_contract_remove(root);
}

#[test]
fn registry_list_rejects_duplicate_or_unsorted_index_identities() {
    for packages in [
        vec![
            PackageSummary {
                name: "cell".to_string(),
                version: "1.0.0".to_string(),
                resource_name: "Cell".to_string(),
                published_at_unix: 1,
                total_bytes: 1,
                package_sha256: "a".repeat(64),
            },
            PackageSummary {
                name: "cell".to_string(),
                version: "1.0.0".to_string(),
                resource_name: "Cell".to_string(),
                published_at_unix: 1,
                total_bytes: 1,
                package_sha256: "a".repeat(64),
            },
        ],
        vec![
            PackageSummary {
                name: "zeta".to_string(),
                version: "1.0.0".to_string(),
                resource_name: "Zeta".to_string(),
                published_at_unix: 1,
                total_bytes: 1,
                package_sha256: "b".repeat(64),
            },
            PackageSummary {
                name: "alpha".to_string(),
                version: "1.0.0".to_string(),
                resource_name: "Alpha".to_string(),
                published_at_unix: 1,
                total_bytes: 1,
                package_sha256: "c".repeat(64),
            },
        ],
    ] {
        let registry = registry_contract_temp_dir("invalid-index-order");
        registry_contract_init(&registry);
        write_registry_index(
            &registry,
            &RegistryIndex {
                schema_version: REGISTRY_SCHEMA_VERSION,
                generated_at_unix: 1,
                packages,
            },
        )
        .expect("write semantic-invalid index");

        list_packages(ListRequest {
            registry_root: registry.clone(),
            token: None,
        })
        .expect_err("duplicate or unsorted index must fail");
        registry_contract_remove(registry);
    }
}

#[test]
fn registry_list_rejects_summary_that_disagrees_with_metadata() {
    let root = registry_contract_temp_dir("index-metadata-disagreement");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    registry_contract_bundle(&bundle, "Cell");
    registry_contract_init(&registry);
    registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish");
    let mut index = registry_contract_index(&registry);
    index.packages[0].total_bytes += 1;
    write_registry_index(&registry, &index).expect("write inconsistent index");

    list_packages(ListRequest {
        registry_root: registry.clone(),
        token: None,
    })
    .expect_err("index summary disagreement must fail");
    registry_contract_remove(root);
}

#[test]
fn registry_verify_rejects_payload_add_remove_resize_and_digest_changes() {
    for mutation in ["add", "remove", "resize", "digest"] {
        let root = registry_contract_temp_dir("payload-mutation");
        let registry = root.join("registry");
        let bundle = root.join("bundle");
        registry_contract_bundle(&bundle, "Cell");
        registry_contract_init(&registry);
        let report =
            registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish");
        let stored_bundle = report.package_root.join("bundle");
        match mutation {
            "add" => fs::write(stored_bundle.join("extra.bin"), b"extra").expect("add payload"),
            "remove" => {
                fs::remove_file(stored_bundle.join("program.stbc")).expect("remove payload")
            }
            "resize" => {
                fs::write(stored_bundle.join("program.stbc"), b"x").expect("resize payload")
            }
            "digest" => {
                let path = stored_bundle.join("program.stbc");
                let mut bytes = registry_contract_read(&path);
                bytes[0] ^= 0x01;
                fs::write(path, bytes).expect("mutate payload");
            }
            _ => unreachable!(),
        }

        verify_package(VerifyRequest {
            registry_root: registry.clone(),
            name: "cell".to_string(),
            version: "1.0.0".to_string(),
            token: None,
        })
        .expect_err("payload mutation must fail verification");
        registry_contract_remove(root);
    }
}

#[test]
fn registry_verify_rejects_metadata_identity_and_total_mismatches() {
    for mutation in ["name", "version", "total"] {
        let root = registry_contract_temp_dir("metadata-mismatch");
        let registry = root.join("registry");
        let bundle = root.join("bundle");
        registry_contract_bundle(&bundle, "Cell");
        registry_contract_init(&registry);
        let report =
            registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish");
        let mut metadata = registry_contract_metadata(&report.package_root);
        match mutation {
            "name" => metadata.name = "other".to_string(),
            "version" => metadata.version = "2.0.0".to_string(),
            "total" => metadata.total_bytes += 1,
            _ => unreachable!(),
        }
        write_json_file(&report.metadata_path, &metadata).expect("write inconsistent metadata");

        verify_package(VerifyRequest {
            registry_root: registry.clone(),
            name: "cell".to_string(),
            version: "1.0.0".to_string(),
            token: None,
        })
        .expect_err("metadata mismatch must fail verification");
        registry_contract_remove(root);
    }
}

#[test]
fn registry_verify_rejects_invalid_digest_encoding_and_duplicate_file_facts() {
    for mutation in ["encoding", "duplicate"] {
        let root = registry_contract_temp_dir("metadata-facts");
        let registry = root.join("registry");
        let bundle = root.join("bundle");
        registry_contract_bundle(&bundle, "Cell");
        registry_contract_init(&registry);
        let report =
            registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish");
        let mut metadata = registry_contract_metadata(&report.package_root);
        match mutation {
            "encoding" => metadata.files[0].sha256 = "NOT-A-SHA256".to_string(),
            "duplicate" => metadata.files.push(metadata.files[0].clone()),
            _ => unreachable!(),
        }
        write_json_file(&report.metadata_path, &metadata).expect("write malformed metadata facts");

        verify_package(VerifyRequest {
            registry_root: registry.clone(),
            name: "cell".to_string(),
            version: "1.0.0".to_string(),
            token: None,
        })
        .expect_err("invalid metadata facts must fail verification");
        registry_contract_remove(root);
    }
}

#[test]
fn registry_download_verification_failure_leaves_empty_destination_empty() {
    let root = registry_contract_temp_dir("download-preflight");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    let output = root.join("output");
    registry_contract_bundle(&bundle, "Cell");
    registry_contract_init(&registry);
    let report = registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish");
    fs::write(report.package_root.join("bundle/program.stbc"), b"tampered")
        .expect("tamper payload");
    fs::create_dir(&output).expect("create empty output");

    download_package(DownloadRequest {
        registry_root: registry.clone(),
        name: "cell".to_string(),
        version: "1.0.0".to_string(),
        output_root: output.clone(),
        token: None,
        verify_before_install: true,
    })
    .expect_err("tampered package download must fail");

    assert!(fs::read_dir(&output).expect("read output").next().is_none());
    registry_contract_remove(root);
}

#[test]
fn registry_download_rejects_nonempty_destination_without_changes() {
    let root = registry_contract_temp_dir("download-nonempty");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    let output = root.join("output");
    registry_contract_bundle(&bundle, "Cell");
    registry_contract_init(&registry);
    registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish");
    fs::create_dir(&output).expect("create output");
    fs::write(output.join("keep.txt"), b"keep").expect("write sentinel");

    download_package(DownloadRequest {
        registry_root: registry.clone(),
        name: "cell".to_string(),
        version: "1.0.0".to_string(),
        output_root: output.clone(),
        token: None,
        verify_before_install: true,
    })
    .expect_err("nonempty output must fail");

    assert_eq!(registry_contract_read(&output.join("keep.txt")), b"keep");
    assert_eq!(fs::read_dir(&output).expect("read output").count(), 1);
    registry_contract_remove(root);
}

#[test]
fn registry_missing_or_malformed_metadata_fails_without_download_mutation() {
    for mutation in ["missing", "malformed"] {
        let root = registry_contract_temp_dir("metadata-unreadable");
        let registry = root.join("registry");
        let bundle = root.join("bundle");
        let output = root.join("output");
        registry_contract_bundle(&bundle, "Cell");
        registry_contract_init(&registry);
        let report =
            registry_contract_publish(&registry, &bundle, "cell", "1.0.0").expect("publish");
        if mutation == "missing" {
            fs::remove_file(&report.metadata_path).expect("remove metadata");
        } else {
            fs::write(&report.metadata_path, b"{not-json").expect("corrupt metadata");
        }

        download_package(DownloadRequest {
            registry_root: registry.clone(),
            name: "cell".to_string(),
            version: "1.0.0".to_string(),
            output_root: output.clone(),
            token: None,
            verify_before_install: true,
        })
        .expect_err("unreadable metadata must fail");
        assert!(!output.exists());
        registry_contract_remove(root);
    }
}

#[cfg(unix)]
#[test]
fn registry_publish_rejects_source_symlink_without_index_or_identity() {
    use std::os::unix::fs::symlink;

    let root = registry_contract_temp_dir("source-symlink");
    let registry = root.join("registry");
    let bundle = root.join("bundle");
    let outside = root.join("outside-secret.txt");
    registry_contract_bundle(&bundle, "Cell");
    fs::write(&outside, b"outside").expect("write outside file");
    symlink(&outside, bundle.join("linked-secret.txt")).expect("create source symlink");
    registry_contract_init(&registry);
    let index_before = registry_contract_read(&registry_index_path(&registry));

    registry_contract_publish(&registry, &bundle, "cell", "1.0.0")
        .expect_err("source symlink must fail publication");

    assert_eq!(
        registry_contract_read(&registry_index_path(&registry)),
        index_before
    );
    assert!(!package_root(&registry, "cell", "1.0.0").exists());
    registry_contract_remove(root);
}

#[cfg(unix)]
#[test]
fn registry_verify_and_download_reject_stored_package_symlink() {
    use std::os::unix::fs::symlink;

    let root = registry_contract_temp_dir("stored-symlink");
    let registry = root.join("registry");
    let outside = root.join("outside");
    let identity = package_root(&registry, "cell", "1.0.0");
    registry_contract_init(&registry);
    fs::create_dir_all(outside.join("bundle")).expect("create outside package");
    fs::write(outside.join("bundle/payload.bin"), b"outside").expect("write outside payload");
    fs::write(
        outside.join(PACKAGE_METADATA_FILE),
        br#"{"name":"cell","version":"1.0.0","resource_name":"Cell","bundle_version":1,"published_at_unix":1,"total_bytes":7,"package_sha256":"invalid","files":[]}"#,
    )
    .expect("write outside metadata");
    fs::create_dir_all(identity.parent().expect("identity parent"))
        .expect("create identity parent");
    symlink(&outside, &identity).expect("create package symlink");

    verify_package(VerifyRequest {
        registry_root: registry.clone(),
        name: "cell".to_string(),
        version: "1.0.0".to_string(),
        token: None,
    })
    .expect_err("stored package symlink must fail verification");
    let output = root.join("output");
    download_package(DownloadRequest {
        registry_root: registry.clone(),
        name: "cell".to_string(),
        version: "1.0.0".to_string(),
        output_root: output.clone(),
        token: None,
        verify_before_install: false,
    })
    .expect_err("stored package symlink must fail unchecked download");
    assert!(!output.exists());
    registry_contract_remove(root);
}
