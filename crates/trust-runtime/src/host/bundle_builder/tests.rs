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

    let report = build_program_stbc(&root, None).expect("build should pass");
    assert!(report.program_path.exists());
    assert!(report.sources.iter().any(|path| path.ends_with("main.st")));
    assert!(report.sources.iter().any(|path| path.ends_with("lib.st")));
    assert_eq!(
        report.resolved_dependencies,
        vec!["LibA".to_string(), "LibB".to_string()]
    );

    let inspection = inspect_project_layout(&root, None).expect("inspection should pass");
    assert!(inspection.sources_root.ends_with("src"));
    assert!(
        inspection
            .manifest_path
            .as_ref()
            .is_some_and(|path| path.ends_with("trust-lsp.toml"))
    );
    assert_eq!(
        inspection.resolved_dependencies,
        vec!["LibA".to_string(), "LibB".to_string()]
    );
    assert_eq!(inspection.dependency_roots.len(), 2);
    assert!(inspection.sources.iter().any(|path| path.ends_with("main.st")));
    assert!(inspection.sources.iter().any(|path| path.ends_with("lib.st")));

    fs::remove_dir_all(root).ok();
}

#[test]
fn build_accepts_cross_file_root_global_struct_field_access() {
    let root = temp_dir("trust-runtime-build-cross-file-global-struct");
    write_cross_file_global_struct_project(&root);

    let report = build_program_stbc(&root, None).expect("build should pass");
    assert!(report.program_path.exists());
    assert!(report.sources.iter().any(|path| path.ends_with("01_types.st")));
    assert!(report.sources.iter().any(|path| path.ends_with("02_globals.st")));
    assert!(report.sources.iter().any(|path| path.ends_with("main.st")));

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

    let err = build_program_stbc(&root, None).expect_err("build should fail");
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

    let err = build_program_stbc(&root, None).expect_err("build should fail");
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

    let err = build_program_stbc(&root, None).expect_err("build should fail");
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

    let first = build_program_stbc(&root, None).expect("first build");
    let first_bytes = fs::read(&first.program_path).expect("read first program");
    let second = build_program_stbc(&root, None).expect("second build");
    let second_bytes = fs::read(&second.program_path).expect("read second program");

    assert_eq!(first.resolved_dependencies, second.resolved_dependencies);
    assert_eq!(first.sources, second.sources);
    assert_eq!(first_bytes, second_bytes);

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
