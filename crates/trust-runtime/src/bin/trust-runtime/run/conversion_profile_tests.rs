//! `trust-runtime run` and the conversion rules of the project's vendor_profile
//! (docs/specs/07-standard-functions.md 2.10). The tests build the project for this computer with
//! the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

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
        "[runtime.control]\n",
        "[runtime.control]\nendpoint = \"tcp://127.0.0.1:0\"\nauth_token = \"trust-ci-token\"\n",
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

/// Builds the project at `root` into its program image for this computer, as `trust-runtime
/// build` does (specification 44 §8.8).
fn build_project(root: &std::path::Path) {
    let toolchain = trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = trust_runtime::native::Target::this_cpu().expect("this computer's CPU");
    trust_runtime::bundle_builder::build_program_image(root, None, target, &toolchain, false, true)
        .expect("build project");
}

// docs/specs/07-standard-functions.md 2.10: `trust-runtime run` runs the project with the
// conversion rules of its manifest's vendor_profile, which its image was built with and records
// (specification 44 §5.2). Worked from the table of 2.10: `REAL_TO_INT(2.5)` is 3 under the CODESYS
// rules (half away from zero) and 2 under the IEC rules (ties to even). Catches a run whose program
// was built with other rules, and a misspelled profile run under the IEC rules.
#[test]
fn project_runtime_uses_the_conversion_rules_of_the_project_vendor_profile() {
    use trust_runtime::stdlib::conversions::ConversionProfile;
    use trust_runtime::value::Value;

    for (name, vendor_profile, expected, rounded) in [
        ("codesys", "codesys", ConversionProfile::Codesys, 3),
        ("beckhoff", "beckhoff", ConversionProfile::Codesys, 3),
        ("twincat", "twincat", ConversionProfile::Codesys, 3),
        ("siemens", "siemens", ConversionProfile::Iec, 2),
    ] {
        let root = vendor_profile_project(name, vendor_profile);
        build_project(&root);
        let mut loaded = super::load_runtime(Some(root.clone()), None, None).expect("load project");
        let image = &loaded.bundle.as_ref().expect("the project's bundle").image;
        assert_eq!(
            trust_runtime_core::native::image::Image::parse(image)
                .expect("the image parses")
                .header()
                .conversion_profile(),
            expected,
            "{vendor_profile}"
        );
        loaded.runtime.execute_cycle().expect("one scan");
        assert_eq!(
            loaded.runtime.read_variable("Main.rounded"),
            Ok(Value::Int(rounded)),
            "{vendor_profile}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    let root = vendor_profile_project("codesys-unknown", "codesys");
    build_project(&root);
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
