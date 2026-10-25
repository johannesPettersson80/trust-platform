use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Builds the project at `root` for this computer, as `trust-runtime build` does: the program image
/// `program.tpi` with its deployment record (specification 44 §8.8, NX-15). The build needs the
/// toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.
fn build(root: &Path) -> anyhow::Result<BundleBuildReport> {
    let toolchain = crate::native::Toolchain::find()?;
    let target = crate::native::Target::this_cpu()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU runs no program image"))?;
    build_program_image(root, None, target, &toolchain, false, true)
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, content).expect("write file");
}

fn write_root_source(root: &Path) {
    write_file(
        &root.join("src/main.st"),
        r#"
PROGRAM Main
VAR
    y : INT;
END_VAR
y := DepDouble(2);
END_PROGRAM
"#,
    );
}

fn write_dependency_source(root: &Path, name: &str) {
    write_file(
        &root.join("src/lib.st"),
        &format!(
            r#"
FUNCTION {name} : INT
VAR_INPUT
    x : INT;
END_VAR
{name} := x * 2;
END_FUNCTION
"#
        ),
    );
}

fn write_cross_file_global_struct_project(root: &Path) {
    write_file(
        &root.join("src/01_types.st"),
        r#"
TYPE CARRIER :
STRUCT
    A : INT;
END_STRUCT
END_TYPE
"#,
    );
    write_file(
        &root.join("src/02_globals.st"),
        r#"
VAR_GLOBAL
    G : CARRIER;
END_VAR
"#,
    );
    write_file(
        &root.join("src/main.st"),
        r#"
PROGRAM Main
VAR
    x : INT;
END_VAR
x := G.A;
END_PROGRAM
"#,
    );
}

#[test]
fn build_includes_transitive_dependency_sources() {
    let root = temp_dir("trust-runtime-build-deps");
    let dep_a = root.join("deps/lib-a");
    let dep_b = root.join("deps/lib-b");
    write_root_source(&root);
    write_dependency_source(&dep_a, "DepDouble");
    write_dependency_source(&dep_b, "DepTriple");
    write_file(
        &root.join("trust-lsp.toml"),
        r#"
[dependencies]
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
    );
    write_file(
        &dep_a.join("trust-lsp.toml"),
        r#"
[package]
version = "1.0.0"

[dependencies]
LibB = { path = "../lib-b", version = "2.0.0" }
"#,
    );
    write_file(
        &dep_b.join("trust-lsp.toml"),
        r#"
[package]
version = "2.0.0"
"#,
    );

    let report = build(&root).expect("build should pass");
    assert!(report.image.exists());
    assert!(report.sources.iter().any(|path| path.ends_with("main.st")));
    assert!(report.sources.iter().any(|path| path.ends_with("lib.st")));
    assert_eq!(
        report.resolved_dependencies,
        vec!["LibA".to_string(), "LibB".to_string()]
    );
    // `build --ci` reports the same: the project's main.st and each dependency's lib.st.
    assert_eq!(report.json["source_count"], serde_json::json!(3));
    assert_eq!(
        report.json["resolved_dependencies"],
        serde_json::json!(["LibA", "LibB"])
    );

    let inspection = inspect_project_layout(&root, None).expect("inspection should pass");
    assert!(inspection.sources_root.ends_with("src"));
    assert!(inspection
        .manifest_path
        .as_ref()
        .is_some_and(|path| path.ends_with("trust-lsp.toml")));
    assert_eq!(
        inspection.resolved_dependencies,
        vec!["LibA".to_string(), "LibB".to_string()]
    );
    assert_eq!(inspection.dependency_roots.len(), 2);
    assert!(inspection
        .sources
        .iter()
        .any(|path| path.ends_with("main.st")));
    assert!(inspection
        .sources
        .iter()
        .any(|path| path.ends_with("lib.st")));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_accepts_cross_file_root_global_struct_field_access() {
    let root = temp_dir("trust-runtime-build-cross-file-global-struct");
    write_cross_file_global_struct_project(&root);

    let report = build(&root).expect("build should pass");
    assert!(report.image.exists());
    assert!(report
        .sources
        .iter()
        .any(|path| path.ends_with("01_types.st")));
    assert!(report
        .sources
        .iter()
        .any(|path| path.ends_with("02_globals.st")));
    assert!(report.sources.iter().any(|path| path.ends_with("main.st")));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_emits_hash_bound_openot_definition_for_the_compiled_sources() {
    let root = temp_dir("trust-runtime-build-openot-definition");
    let library_sources = [
        (
            "openot_control_block.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_control_block.st.txt"),
        ),
        (
            "openot_crc32c.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_crc32c.st.txt"),
        ),
        (
            "openot_lifecycle.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_lifecycle.st.txt"),
        ),
        (
            "openot_message.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_message.st.txt"),
        ),
        (
            "openot_producer.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_producer.st.txt"),
        ),
        (
            "openot_records_dropped.st",
            include_str!(
                "../../../tests/fixtures/openot/iec61131/src/openot_records_dropped.st.txt"
            ),
        ),
        (
            "openot_ring256_producer.st",
            include_str!(
                "../../../tests/fixtures/openot/iec61131/src/openot_ring256_producer.st.txt"
            ),
        ),
        (
            "openot_ring.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_ring.st.txt"),
        ),
        (
            "openot_source_high_water.st",
            include_str!(
                "../../../tests/fixtures/openot/iec61131/src/openot_source_high_water.st.txt"
            ),
        ),
        (
            "openot_value_state.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_value_state.st.txt"),
        ),
        (
            "openot_wire_encode.st",
            include_str!("../../../tests/fixtures/openot/iec61131/src/openot_wire_encode.st.txt"),
        ),
    ];
    for (name, source) in library_sources {
        write_file(&root.join("src").join(name), source);
    }
    write_file(
        &root.join("src/main.st"),
        r#"
PROGRAM Main
VAR
    Start : BOOL {attribute 'oot' := 'message', 'template' := 'bundle proof'};
END_VAR
Start := TRUE;
END_PROGRAM
"#,
    );

    build(&root).expect("build should pass");

    let definition_path = root.join("openot-definition.json");
    let bytes = fs::read(&definition_path).expect("build must emit OpenOT definition");
    let definition: open_ot_definition::DefinitionFile =
        serde_json::from_slice(&bytes).expect("emitted definition must parse");
    let hash = open_ot_definition::compute_content_hash(&definition)
        .expect("emitted definition hash must verify");
    assert_eq!(definition.header.content_hash, hash.content_hash);
    assert!(definition
        .message_templates
        .iter()
        .any(|template| template.format == "bundle proof"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn check_compiles_without_writing_the_program_image() {
    let root = temp_dir("trust-runtime-check-no-write");
    write_root_source(&root);
    write_dependency_source(&root.join("deps/lib-a"), "DepDouble");
    write_file(
        &root.join("trust-lsp.toml"),
        r#"
[dependencies]
LibA = "deps/lib-a"
"#,
    );

    let report = check_program(&root, None).expect("check should pass");
    assert!(report.sources.iter().any(|path| path.ends_with("main.st")));
    assert_eq!(report.resolved_dependencies, vec!["LibA".to_string()]);
    assert!(
        !root.join(crate::native::PROGRAM_IMAGE_FILE).exists(),
        "check must not write the program image"
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_fails_for_missing_dependency_path() {
    let root = temp_dir("trust-runtime-build-missing");
    write_root_source(&root);
    write_file(
        &root.join("trust-lsp.toml"),
        r#"
[dependencies]
Missing = "deps/missing"
"#,
    );

    let err = build(&root).expect_err("build should fail");
    let message = err.to_string();
    assert!(message.contains("dependency 'Missing' path does not exist"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_fails_for_cyclic_dependencies() {
    let root = temp_dir("trust-runtime-build-cycle");
    let dep_a = root.join("deps/lib-a");
    let dep_b = root.join("deps/lib-b");
    write_root_source(&root);
    write_dependency_source(&dep_a, "DepDouble");
    write_dependency_source(&dep_b, "DepTriple");
    write_file(
        &root.join("trust-lsp.toml"),
        r#"
[dependencies]
LibA = { path = "deps/lib-a" }
"#,
    );
    write_file(
        &dep_a.join("trust-lsp.toml"),
        r#"
[dependencies]
LibB = { path = "../lib-b" }
"#,
    );
    write_file(
        &dep_b.join("trust-lsp.toml"),
        r#"
[dependencies]
LibA = { path = "../lib-a" }
"#,
    );

    let err = build(&root).expect_err("build should fail");
    let message = err.to_string();
    assert!(message.contains("cyclic dependency detected"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_fails_for_version_mismatch() {
    let root = temp_dir("trust-runtime-build-version");
    let dep_a = root.join("deps/lib-a");
    write_root_source(&root);
    write_dependency_source(&dep_a, "DepDouble");
    write_file(
        &root.join("trust-lsp.toml"),
        r#"
[dependencies]
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
    );
    write_file(
        &dep_a.join("trust-lsp.toml"),
        r#"
[package]
version = "2.0.0"
"#,
    );

    let err = build(&root).expect_err("build should fail");
    let message = err.to_string();
    assert!(message.contains("requested version 1.0.0"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn dependency_resolution_order_is_deterministic() {
    let root = temp_dir("trust-runtime-build-deterministic");
    let dep_a = root.join("deps/lib-a");
    let dep_b = root.join("deps/lib-b");
    write_file(
        &root.join("src/main.st"),
        r#"
PROGRAM Main
VAR
    a : INT;
    b : INT;
END_VAR
a := ADouble(1);
b := BDouble(2);
END_PROGRAM
"#,
    );
    write_dependency_source(&dep_a, "ADouble");
    write_dependency_source(&dep_b, "BDouble");
    write_file(
        &root.join("trust-lsp.toml"),
        r#"
[dependencies]
LibB = { path = "deps/lib-b", version = "1.0.0" }
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
    );
    write_file(
        &dep_a.join("trust-lsp.toml"),
        r#"
[package]
version = "1.0.0"
"#,
    );
    write_file(
        &dep_b.join("trust-lsp.toml"),
        r#"
[package]
version = "1.0.0"
"#,
    );

    let first = build(&root).expect("first build");
    let first_bytes =
        fs::read(generated_output_path!(&first.image, &root)).expect("read first program");
    let second = build(&root).expect("second build");
    let second_bytes =
        fs::read(generated_output_path!(&second.image, &root)).expect("read second program");

    assert_eq!(first.resolved_dependencies, second.resolved_dependencies);
    assert_eq!(first.sources, second.sources);
    assert_eq!(reproducible(&first_bytes), reproducible(&second_bytes));

    fs::remove_dir_all(root).ok();
}

#[test]
fn resolve_sources_root_prefers_src_directory() {
    let root = temp_dir("trust-runtime-resolve-src");
    write_file(&root.join("src/main.st"), "PROGRAM Main END_PROGRAM");

    let resolved = resolve_sources_root(&root, None).expect("resolve sources root");
    assert!(resolved.ends_with("src"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn resolve_sources_root_rejects_legacy_sources_directory() {
    let root = temp_dir("trust-runtime-resolve-sources");
    write_file(&root.join("sources/main.st"), "PROGRAM Legacy END_PROGRAM");
    let err = resolve_sources_root(&root, None).expect_err("legacy sources should fail");
    let message = err.to_string();
    assert!(message.contains("missing src/ directory"));

    fs::remove_dir_all(root).ok();
}

#[test]
fn native_build_labels_each_source_relative_to_the_project_root() {
    // Specification 12 §7.5.1.4 (kept by specification 44 §5.3) and specification 13: a file's
    // label is its path relative to the project root with `/` separators, and a client names the
    // file by a path that ends with it. On the C6 bench of 24 October 2026 the native image carried
    // the build computer's absolute path, so `breakpoints.set` with `src/main.st` was refused as an
    // unknown source path. Worked by hand: `<root>/src/main.st` -> `src/main.st` and
    // `<root>/src/motors/pump.st` -> `src/motors/pump.st`. Catches an absolute label and a label
    // with the platform's separator.
    let root = temp_dir("trust-native-labels");
    write_root_source(&root);
    write_file(
        &root.join("src/motors/pump.st"),
        "FUNCTION DepDouble : INT\nVAR_INPUT x : INT; END_VAR\nDepDouble := x * 2;\nEND_FUNCTION\n",
    );
    let sources = collect_project_sources_labelled(&root, None).expect("sources");
    let labels: Vec<&str> = sources
        .iter()
        .map(|source| source.path.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(labels, ["src/main.st", "src/motors/pump.st"]);
    let _ = fs::remove_dir_all(&root);
}

/// Specification 11 §6.7.1: an image runs the one resource it was built from, so a project whose
/// `runtime.toml` names another resource than its program's `RESOURCE` is not built, and the names
/// compare without regard to case. Catches a build that writes an image the project then runs under
/// a resource its program does not declare.
#[test]
fn build_refuses_a_runtime_resource_that_is_not_the_program_resource() {
    let root = temp_dir("trust-runtime-build-resource");
    write_file(
        &root.join("src/main.st"),
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION Plant
RESOURCE Line ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
    write_file(
        &root.join("runtime.toml"),
        &crate::bundle_template::render_runtime_toml(&"Other".into(), 10),
    );
    let error = build(&root).expect_err("another resource is refused");
    assert!(
        error
            .to_string()
            .contains("resource 'Other' of runtime.toml is not the RESOURCE the program declares"),
        "{error}"
    );
    assert!(
        !root.join(crate::native::PROGRAM_IMAGE_FILE).exists(),
        "a refused build writes no program"
    );

    write_file(
        &root.join("runtime.toml"),
        &crate::bundle_template::render_runtime_toml(&"LINE".into(), 10),
    );
    build(&root).expect("the program's RESOURCE builds in any case");
    fs::remove_dir_all(root).ok();
}

/// The bytes of program image `image` that two builds of one project share (specification 44
/// NX-07): all but the build time in the header, the header's checksum over it and the signature.
fn reproducible(image: &[u8]) -> Vec<u8> {
    use trust_runtime_core::native::image::{Image, SectionId};
    let mut bytes = image.to_vec();
    bytes[8..12].fill(0);
    bytes[24..32].fill(0);
    if let Some(sign) = Image::parse(image)
        .expect("the program image parses")
        .section(SectionId::Sign)
    {
        let start = sign.offset as usize;
        bytes[start..start + sign.length as usize].fill(0);
    }
    bytes
}
