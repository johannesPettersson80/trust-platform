use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

fn temp_dir(name: &str) -> PathBuf {
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "trust-deploy-{name}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("create deploy test directory");
    path
}

#[test]
fn default_bundle_labels_are_unique_for_consecutive_deployments() {
    let labels = (0..64)
        .map(|_| default_bundle_label())
        .collect::<HashSet<_>>();
    assert_eq!(labels.len(), 64);
}

#[test]
fn deployment_labels_are_portable_single_components() {
    let generated = default_bundle_label();
    validate_bundle_label(&generated).expect("generated label is valid");
    validate_bundle_label("release-2026.07.22").expect("portable explicit label is valid");

    for invalid in [
        "",
        ".",
        "..",
        "../escaped",
        "nested/name",
        "nested\\name",
        "/absolute",
        "C:prefix",
    ] {
        assert!(
            validate_bundle_label(invalid).is_err(),
            "invalid deployment label accepted: {invalid:?}"
        );
    }
}

#[test]
fn deployed_sidecar_paths_require_normalized_relative_components() {
    validate_bundle_relative_path(Path::new("config/ads.toml"), "sidecar")
        .expect("normalized nested path is valid");

    for invalid in [
        "",
        "/absolute.toml",
        "../escaped.toml",
        "config/../escaped.toml",
        "./ads.toml",
        "config/./ads.toml",
        "config//ads.toml",
        "config\\ads.toml",
        "C:ads.toml",
    ] {
        assert!(
            validate_bundle_relative_path(Path::new(invalid), "sidecar").is_err(),
            "non-normalized sidecar path accepted: {invalid:?}"
        );
    }
}

#[test]
fn bundle_targets_include_current_and_immediate_previous() {
    let current = PathBuf::from("/deploy/bundles/current");
    let previous = PathBuf::from("/deploy/bundles/previous");
    assert_eq!(
        bundle_targets(&current, Some(&previous)),
        vec![current, previous]
    );
}

#[test]
fn prune_bundles_keeps_only_selected_directories() {
    let root = temp_dir("prune");
    let bundles = root.join("bundles");
    let current = bundles.join("current");
    let previous = bundles.join("previous");
    let stale = bundles.join("stale");
    for path in [&current, &previous, &stale] {
        fs::create_dir_all(path).expect("create bundle directory");
    }

    prune_bundles(&bundles, &bundle_targets(&current, Some(&previous))).expect("prune bundles");

    assert!(current.is_dir());
    assert!(previous.is_dir());
    assert!(!stale.exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn read_link_target_resolves_relative_to_the_link_directory() {
    let root = temp_dir("relative-link");
    let bundle = root.join("bundles/version-1");
    fs::create_dir_all(&bundle).expect("create bundle target");
    let link = root.join("current");
    std::os::unix::fs::symlink("bundles/version-1", &link).expect("create relative link");

    assert_eq!(
        read_link_target(&link)
            .expect("read deployment pointer")
            .expect("deployment link exists"),
        bundle
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn update_symlink_replaces_a_dangling_pointer() {
    let root = temp_dir("dangling-link");
    let bundle = root.join("bundles/version-1");
    fs::create_dir_all(&bundle).expect("create bundle target");
    let link = root.join("current");
    std::os::unix::fs::symlink(root.join("missing"), &link).expect("create dangling link");

    update_symlink(&link, &bundle).expect("replace dangling deployment link");

    assert_eq!(
        fs::canonicalize(&link).expect("resolve replacement link"),
        fs::canonicalize(&bundle).expect("resolve bundle")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn update_symlink_does_not_overwrite_an_ordinary_file() {
    let root = temp_dir("ordinary-pointer-file");
    let bundle = root.join("bundles/version-1");
    fs::create_dir_all(&bundle).expect("create bundle target");
    let link = root.join("current");
    fs::write(&link, "operator-owned file\n").expect("write ordinary file");

    let error = update_symlink(&link, &bundle).expect_err("ordinary file must not be replaced");

    assert!(error.to_string().contains("not a symbolic link"));
    assert_eq!(
        fs::read_to_string(&link).expect("ordinary file remains"),
        "operator-owned file\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn safe_state_comparison_uses_structural_addresses() {
    let byte_address = trust_runtime::io::IoAddress::parse("%QB0").expect("parse byte address");
    let mut byte_array_address = byte_address.clone();
    byte_array_address.size = trust_runtime::io::IoSize::Bytes(2);
    let previous = trust_runtime::io::IoSafeState {
        outputs: vec![(byte_address, trust_runtime::value::Value::Byte(0))],
    };
    let next = trust_runtime::io::IoSafeState {
        outputs: vec![(byte_array_address, trust_runtime::value::Value::Byte(0))],
    };

    assert!(
        safe_state_changed(&previous, &next),
        "different address sizes must not collapse to the same display string"
    );
}

#[test]
fn source_collection_handles_glob_metacharacters_in_project_paths() {
    let root = temp_dir("sources-[literal]");
    let nested = root.join("src/nested");
    fs::create_dir_all(&nested).expect("create nested source directory");
    fs::write(root.join("src/Main.st"), "PROGRAM Main END_PROGRAM\n").expect("write root source");
    fs::write(
        nested.join("Helper.POU"),
        "FUNCTION Helper : BOOL END_FUNCTION\n",
    )
    .expect("write nested source");

    let sources = collect_sources(&root).expect("collect sources from literal path");

    assert_eq!(
        sources.keys().cloned().collect::<Vec<_>>(),
        vec!["Main.st".to_string(), "nested/Helper.POU".to_string()]
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn source_diff_propagates_invalid_source_directory_errors() {
    let root = temp_dir("sources-not-directory");
    fs::write(root.join("src"), "not a directory\n").expect("write invalid source path");

    let error = diff_sources(None, &root).expect_err("invalid source directory must fail");

    assert!(
        error.to_string().contains("source path is not a directory"),
        "unexpected error: {error:#}"
    );
    let _ = fs::remove_dir_all(root);
}
