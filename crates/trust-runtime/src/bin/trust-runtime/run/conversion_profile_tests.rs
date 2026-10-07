//! `trust-runtime run` and the conversion rules of the project's vendor_profile
//! (docs/specs/07-standard-functions.md 2.10).

/// A runnable project folder whose `trust-lsp.toml` sets `vendor_profile`.
fn vendor_profile_project(name: &str, vendor_profile: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-run-vendor-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).expect("create project");
    let runtime_toml =
        trust_runtime::bundle_template::render_runtime_toml(&smol_str::SmolStr::new("main"), 100);
    #[cfg(windows)]
    let runtime_toml = runtime_toml.replacen(
        "endpoint = \"unix:///tmp/trust-runtime.sock\"",
        "endpoint = \"tcp://127.0.0.1:0\"\nauth_token = \"trust-ci-token\"",
        1,
    );
    std::fs::write(root.join("runtime.toml"), runtime_toml).expect("write runtime.toml");
    let io = trust_runtime::bundle_template::build_io_config_auto("loopback").expect("io template");
    std::fs::write(
        root.join("io.toml"),
        trust_runtime::bundle_template::render_io_toml(&io),
    )
    .expect("write io.toml");
    std::fs::write(
        root.join("src").join("main.st"),
        "PROGRAM Main\nVAR\n    half : REAL := 2.5;\n    rounded : INT;\nEND_VAR\nrounded := REAL_TO_INT(half);\nEND_PROGRAM\n",
    )
    .expect("write main.st");
    std::fs::write(
        root.join("trust-lsp.toml"),
        format!("[project]\nvendor_profile = \"{vendor_profile}\"\n"),
    )
    .expect("write trust-lsp.toml");
    root
}

// docs/specs/07-standard-functions.md 2.10: `trust-runtime run` compiles the project with the
// conversion rules of its manifest's vendor_profile.
#[test]
fn project_runtime_uses_the_conversion_rules_of_the_project_vendor_profile() {
    use trust_runtime::stdlib::conversions::ConversionProfile;

    for (name, vendor_profile, expected) in [
        ("codesys", "codesys", ConversionProfile::Codesys),
        ("siemens", "siemens", ConversionProfile::Iec),
    ] {
        let root = vendor_profile_project(name, vendor_profile);
        trust_runtime::bundle_builder::build_program_stbc(&root, None).expect("build project");
        let loaded = super::load_runtime(Some(root.clone()), None, None).expect("load project");
        assert_eq!(
            loaded.runtime.stdlib().conversion_profile(),
            expected,
            "{vendor_profile}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    let root = vendor_profile_project("codesys-unknown", "codesys");
    trust_runtime::bundle_builder::build_program_stbc(&root, None).expect("build project");
    std::fs::write(
        root.join("trust-lsp.toml"),
        "[project]\nvendor_profile = \"codesis\"\n",
    )
    .expect("rewrite trust-lsp.toml");
    let error = super::load_runtime(Some(root.clone()), None, None)
        .err()
        .expect("unknown vendor profile")
        .to_string();
    assert!(
        error.contains("unknown vendor_profile 'codesis'"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(root);
}
