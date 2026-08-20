use super::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use trust_wasm_analysis::{ByteSpan, Position, Range, RelatedInfoItem};

static NEXT_PROJECT_ID: AtomicUsize = AtomicUsize::new(0);

struct TestProject {
    root: PathBuf,
}

impl TestProject {
    fn new(label: &str) -> Self {
        let id = NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-dev-workflow-contract-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src")).expect("create test project");
        Self {
            root: root.canonicalize().expect("canonicalize test project"),
        }
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, text).expect("write fixture");
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.root.join(relative)).expect("read fixture")
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn valid_program(name: &str) -> String {
    format!("PROGRAM {name}\nEND_PROGRAM\n")
}

fn invalid_program(name: &str, symbol: &str) -> String {
    format!("PROGRAM {name}\nVAR\n  value : INT;\nEND_VAR\nvalue := {symbol};\nEND_PROGRAM\n")
}

fn issue_files(payload: &Value) -> Vec<&str> {
    payload["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .map(|issue| issue["file"].as_str().expect("issue file"))
        .collect()
}

#[test]
fn project_source_discovery_accepts_every_ascii_case_extension_spelling() {
    let project = TestProject::new("extension-cases");
    let extensions = [
        "st", "sT", "St", "ST", "pou", "poU", "pOu", "pOU", "Pou", "PoU", "POu", "POU",
    ];
    for (index, extension) in extensions.iter().enumerate() {
        project.write(
            &format!("src/source-{index}.{extension}"),
            &valid_program(&format!("P{index}")),
        );
    }

    let paths = collect_project_source_paths(&project.root, None).expect("discover sources");

    assert_eq!(paths.len(), extensions.len(), "{paths:#?}");
    for (index, extension) in extensions.iter().enumerate() {
        assert!(
            paths.contains(&format!("src/source-{index}.{extension}")),
            "missing .{extension}: {paths:#?}"
        );
    }
}

#[test]
fn project_source_discovery_treats_root_metacharacters_literally() {
    let project = TestProject::new("literal-[batch]-star*");
    project.write("src/main.st", &valid_program("Main"));

    let paths = collect_project_source_paths(&project.root, None).expect("discover literal root");

    assert_eq!(paths, vec!["src/main.st"]);
}

#[test]
fn project_source_discovery_is_recursive_deduplicated_and_sorted() {
    let project = TestProject::new("recursive-order");
    project.write("src/z/last.st", &valid_program("Last"));
    project.write("src/a/first.POU", &valid_program("First"));
    project.write("src/middle.ST", &valid_program("Middle"));

    let paths = collect_project_source_paths(&project.root, None).expect("discover sources");

    assert_eq!(
        paths,
        vec!["src/a/first.POU", "src/middle.ST", "src/z/last.st"]
    );
}

#[test]
fn project_source_discovery_ignores_unsupported_files_and_directories() {
    let project = TestProject::new("regular-files");
    project.write("src/main.st", &valid_program("Main"));
    project.write("src/notes.txt", "not structured text");
    std::fs::create_dir_all(project.root.join("src/not-a-file.pou"))
        .expect("create source-looking directory");

    let paths = collect_project_source_paths(&project.root, None).expect("discover sources");

    assert_eq!(paths, vec!["src/main.st"]);
}

#[test]
fn project_source_discovery_honors_explicit_source_root() {
    let project = TestProject::new("source-override");
    project.write("src/default.st", &valid_program("Default"));
    project.write("alternate/selected.st", &valid_program("Selected"));

    let paths = collect_project_source_paths(
        &project.root,
        Some(project.root.join("alternate").as_path()),
    )
    .expect("discover override");

    assert_eq!(paths, vec!["alternate/selected.st"]);
}

#[cfg(unix)]
#[test]
fn project_source_discovery_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let project = TestProject::new("source-symlink");
    let outside = TestProject::new("source-symlink-outside");
    outside.write("secret.st", &valid_program("Secret"));
    symlink(
        outside.root.join("secret.st"),
        project.root.join("src/escape.st"),
    )
    .expect("create escaping source symlink");

    let error = collect_project_source_paths(&project.root, None)
        .expect_err("escaping source alias must fail");

    assert!(
        error
            .to_string()
            .contains("does not live under project root"),
        "{error:#}"
    );
}

#[cfg(unix)]
#[test]
fn project_source_discovery_rejects_symlink_escape_before_target_extension_filtering() {
    use std::os::unix::fs::symlink;

    let project = TestProject::new("source-symlink-unsupported-escape");
    let outside = TestProject::new("source-symlink-unsupported-escape-outside");
    outside.write("secret.txt", "not structured text");
    symlink(
        outside.root.join("secret.txt"),
        project.root.join("src/escape.st"),
    )
    .expect("create escaping source symlink");

    let error = collect_project_source_paths(&project.root, None)
        .expect_err("every escaping source alias must fail before target filtering");

    assert!(
        error
            .to_string()
            .contains("does not live under project root"),
        "{error:#}"
    );
}

#[cfg(unix)]
#[test]
fn project_source_discovery_accepts_contained_source_symlink() {
    use std::os::unix::fs::symlink;

    let project = TestProject::new("contained-source-symlink");
    project.write("shared/target.st", &valid_program("Target"));
    symlink("../shared/target.st", project.root.join("src/alias.st"))
        .expect("create contained source symlink");

    let paths = collect_project_source_paths(&project.root, None).expect("discover sources");

    assert_eq!(paths, vec!["shared/target.st"]);
}

#[cfg(unix)]
#[test]
fn project_source_discovery_ignores_source_alias_to_unsupported_target() {
    use std::os::unix::fs::symlink;

    let project = TestProject::new("unsupported-source-symlink-target");
    project.write("src/target.txt", "not structured text");
    symlink("target.txt", project.root.join("src/alias.st"))
        .expect("create unsupported source symlink");

    let paths = collect_project_source_paths(&project.root, None).expect("discover sources");

    assert!(
        paths.is_empty(),
        "unsupported target was analyzed: {paths:?}"
    );
}

#[test]
fn diagnostics_rejects_content_without_path() {
    let project = TestProject::new("diagnostics-content-path");

    let error = diagnostics_payload(&project.root, None, None, Some(valid_program("Inline")))
        .expect_err("content without identity must fail");

    assert!(
        error
            .to_string()
            .contains("content overrides require a file path"),
        "{error:#}"
    );
}

#[test]
fn diagnostics_inline_override_does_not_mutate_disk() {
    let project = TestProject::new("diagnostics-preview");
    let disk = valid_program("Disk");
    project.write("src/main.st", &disk);

    let payload = diagnostics_payload(
        &project.root,
        None,
        Some("src/main.st"),
        Some(invalid_program("Inline", "Missing")),
    )
    .expect("analyze override");

    assert_eq!(project.read("src/main.st"), disk);
    assert!(payload["errors"].as_u64().expect("error count") > 0);
    assert_eq!(payload["issues"][0]["path"], "src/main.st");
    assert_eq!(
        Path::new(payload["target"].as_str().expect("target")),
        project.root.join("src/main.st")
    );
}

#[test]
fn diagnostics_whole_project_uses_canonical_project_target() {
    let project = TestProject::new("diagnostics-project-target");
    project.write(
        "src/main.st",
        r#"CONFIGURATION Plant
TASK MainTask (PRIORITY := 1);
PROGRAM MainInstance WITH MainTask : Main;
END_CONFIGURATION
PROGRAM Main
END_PROGRAM
"#,
    );

    let payload =
        diagnostics_payload(&project.root, None, None, None).expect("analyze project sources");

    assert_eq!(
        Path::new(payload["target"].as_str().expect("target")),
        project.root
    );
    assert_eq!(payload["errors"], 0, "{payload:#}");
    assert_eq!(payload["warnings"], 0, "{payload:#}");
    assert_eq!(payload["issues"], json!([]));
}

#[test]
fn diagnostics_whole_project_honors_source_root_override() {
    let project = TestProject::new("diagnostics-source-root");
    project.write("src/ignored.st", &invalid_program("Ignored", "MissingA"));
    project.write(
        "selected/checked.st",
        &invalid_program("Checked", "MissingB"),
    );

    let payload = diagnostics_payload(
        &project.root,
        Some(project.root.join("selected").as_path()),
        None,
        None,
    )
    .expect("analyze selected sources");

    let files = issue_files(&payload);
    assert!(!files.is_empty(), "{payload:#}");
    assert!(files
        .iter()
        .all(|file| file.ends_with("selected/checked.st")));
}

#[test]
fn diagnostics_whole_project_orders_issues_by_file_then_position() {
    let project = TestProject::new("diagnostics-order");
    project.write("src/z.st", &invalid_program("Zed", "MissingZ"));
    project.write("src/a.st", &invalid_program("Alpha", "MissingA"));

    let payload =
        diagnostics_payload(&project.root, None, None, None).expect("analyze project sources");
    let files = issue_files(&payload);

    assert!(!files.is_empty(), "{payload:#}");
    assert!(
        files.windows(2).all(|pair| pair[0] <= pair[1]),
        "{files:#?}"
    );
}

#[test]
fn diagnostics_whole_project_includes_mixed_case_extension() {
    let project = TestProject::new("diagnostics-mixed-extension");
    project.write("src/invalid.sT", &invalid_program("Mixed", "Missing"));

    let payload =
        diagnostics_payload(&project.root, None, None, None).expect("analyze project sources");

    assert!(
        payload["errors"].as_u64().expect("error count") > 0,
        "{payload:#}"
    );
    assert!(
        issue_files(&payload)
            .iter()
            .any(|file| file.ends_with("src/invalid.sT")),
        "{payload:#}"
    );
}

#[test]
fn diagnostic_projection_uses_public_coordinates_and_preserves_related_evidence() {
    let diagnostic = DiagnosticItem {
        code: "E-DEMO".to_string(),
        severity: "ERROR".to_string(),
        message: "primary".to_string(),
        range: Range {
            start: Position {
                line: 2,
                character: 4,
            },
            end: Position {
                line: 2,
                character: 9,
            },
        },
        span: ByteSpan { start: 20, end: 25 },
        related: vec![RelatedInfoItem {
            range: Range {
                start: Position {
                    line: 0,
                    character: 1,
                },
                end: Position {
                    line: 0,
                    character: 3,
                },
            },
            span: ByteSpan { start: 1, end: 3 },
            message: "origin".to_string(),
        }],
    };

    let projected = serde_json::to_value(map_diagnostic(
        "src/main.st",
        "/project/src/main.st",
        diagnostic,
    ))
    .expect("serialize issue");

    assert_eq!(
        projected,
        json!({
            "path": "src/main.st",
            "file": "/project/src/main.st",
            "line": 3,
            "column": 5,
            "endLine": 3,
            "endColumn": 10,
            "span": {"start": 20, "end": 25},
            "severity": "error",
            "message": "primary",
            "code": "E-DEMO",
            "source": "trust-analysis",
            "related": [{
                "line": 1,
                "column": 2,
                "endLine": 1,
                "endColumn": 4,
                "span": {"start": 1, "end": 3},
                "message": "origin"
            }]
        })
    );
}

#[test]
fn diagnostic_projection_omits_blank_code_and_empty_related_list() {
    let diagnostic = DiagnosticItem {
        code: "   ".to_string(),
        severity: "Warning".to_string(),
        message: "warning".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 1,
            },
        },
        span: ByteSpan { start: 0, end: 1 },
        related: Vec::new(),
    };

    let projected = serde_json::to_value(map_diagnostic("main.st", "/main.st", diagnostic))
        .expect("serialize issue");

    assert!(projected.get("code").is_none(), "{projected:#}");
    assert!(projected.get("related").is_none(), "{projected:#}");
    assert_eq!(projected["severity"], "warning");
}

#[test]
fn canonicalize_requires_inline_content_or_path() {
    let project = TestProject::new("ast-missing-source");

    let error =
        ast_canonicalize_payload(&project.root, None, None).expect_err("missing source must fail");

    assert!(
        error
            .to_string()
            .contains("canonical AST requires either inline content"),
        "{error:#}"
    );
}

#[test]
fn canonicalize_inline_content_does_not_require_existing_path() {
    let project = TestProject::new("ast-inline");

    let payload = ast_canonicalize_payload(
        &project.root,
        Some("src/not-created.st"),
        Some(valid_program("Inline")),
    )
    .expect("canonicalize inline source");

    assert_eq!(payload["algorithm"], "canonical_ast_jaccard_5gram_v1");
    assert_eq!(payload["gramSize"], 5);
    assert_eq!(payload["parseErrorCount"], 0);
    assert!(!project.root.join("src/not-created.st").exists());
}

#[test]
fn canonicalize_inline_content_takes_precedence_over_disk() {
    let project = TestProject::new("ast-inline-precedence");
    project.write("src/main.st", "PROGRAM Broken\n");

    let payload = ast_canonicalize_payload(
        &project.root,
        Some("src/main.st"),
        Some(valid_program("Inline")),
    )
    .expect("canonicalize override");

    assert_eq!(payload["parseErrorCount"], 0);
    assert_eq!(project.read("src/main.st"), "PROGRAM Broken\n");
}

#[test]
fn canonicalize_disk_source_matches_shared_algorithm() {
    let project = TestProject::new("ast-disk");
    let source = valid_program("Disk");
    project.write("src/main.st", &source);

    let payload = ast_canonicalize_payload(&project.root, Some("src/main.st"), None)
        .expect("canonicalize disk source");
    let expected = serde_json::to_value(canonical_ast_summary(&source)).expect("expected payload");

    assert_eq!(payload, expected);
}

#[test]
fn canonicalize_reports_parse_errors_without_hiding_evidence() {
    let project = TestProject::new("ast-parse-error");

    let payload =
        ast_canonicalize_payload(&project.root, None, Some("PROGRAM Broken\n".to_string()))
            .expect("canonicalize malformed source");

    assert!(
        payload["parseErrorCount"]
            .as_u64()
            .expect("parse error count")
            > 0,
        "{payload:#}"
    );
    assert!(payload["stream"].is_array());
    assert!(payload["fiveGrams"].is_array());
}

#[test]
fn canonicalize_erases_trivia_names_and_literal_values() {
    let project = TestProject::new("ast-normalization");
    let left = "PROGRAM Main\nVAR Counter : INT; END_VAR\n(* note *)\nCounter := 1;\nEND_PROGRAM\n";
    let right =
        "PROGRAM Demo\nVAR Value : INT; END_VAR\n// another note\nValue := 99;\nEND_PROGRAM\n";

    let left = ast_canonicalize_payload(&project.root, None, Some(left.to_string()))
        .expect("canonicalize left");
    let right = ast_canonicalize_payload(&project.root, None, Some(right.to_string()))
        .expect("canonicalize right");

    assert_eq!(left["stream"], right["stream"]);
    assert_eq!(left["fiveGrams"], right["fiveGrams"]);
}

#[test]
fn canonical_five_grams_are_sorted_and_duplicate_free() {
    let project = TestProject::new("ast-gram-set");
    let payload = ast_canonicalize_payload(
        &project.root,
        None,
        Some(
            "PROGRAM Main\nVAR x : INT; END_VAR\nx := x + 1;\nx := x + 1;\nEND_PROGRAM\n"
                .to_string(),
        ),
    )
    .expect("canonicalize source");
    let grams = payload["fiveGrams"].as_array().expect("five grams");
    let values = grams
        .iter()
        .map(|gram| gram.as_str().expect("gram"))
        .collect::<Vec<_>>();

    assert!(
        values.windows(2).all(|pair| pair[0] < pair[1]),
        "{values:#?}"
    );
}

#[test]
fn similarity_requires_both_effective_sources() {
    let project = TestProject::new("similarity-missing-side");

    let error =
        ast_similarity_payload(&project.root, None, Some(valid_program("Left")), None, None)
            .expect_err("missing right input must fail");

    assert!(error.to_string().contains("right AST input"), "{error:#}");
}

#[test]
fn similarity_allows_independent_disk_and_inline_sources() {
    let project = TestProject::new("similarity-source-selection");
    project.write("src/left.st", &valid_program("Disk"));

    let payload = ast_similarity_payload(
        &project.root,
        Some("src/left.st"),
        None,
        Some("src/not-created.st"),
        Some(valid_program("Inline")),
    )
    .expect("compare disk and inline");

    assert_eq!(payload["score"], 1.0);
    assert_eq!(payload["threshold070"], true);
    assert_eq!(payload["threshold095"], true);
    assert!(!project.root.join("src/not-created.st").exists());
}

#[test]
fn similarity_of_short_empty_gram_sets_is_exact() {
    let project = TestProject::new("similarity-empty-grams");

    let payload = ast_similarity_payload(
        &project.root,
        None,
        Some(String::new()),
        None,
        Some(String::new()),
    )
    .expect("compare empty sources");

    assert_eq!(payload["score"], 1.0);
    assert_eq!(payload["sharedGrams"], 0);
    assert_eq!(payload["leftGrams"], 0);
    assert_eq!(payload["rightGrams"], 0);
}

#[test]
fn similarity_reports_structural_difference_and_unique_gram_counts() {
    let project = TestProject::new("similarity-difference");
    let left = "PROGRAM Main\nVAR x : INT; END_VAR\nx := x + 1;\nEND_PROGRAM\n".to_string();
    let right =
        "PROGRAM Main\nVAR x : INT; END_VAR\nIF x > 0 THEN\nx := x + 1;\nEND_IF\nEND_PROGRAM\n"
            .to_string();

    let payload = ast_similarity_payload(&project.root, None, Some(left), None, Some(right))
        .expect("compare structures");

    assert!(
        payload["score"].as_f64().expect("score") < 0.70,
        "{payload:#}"
    );
    assert_eq!(payload["threshold070"], false);
    assert_eq!(payload["threshold095"], false);
    assert!(payload["leftGrams"].as_u64().expect("left grams") > 0);
    assert!(payload["rightGrams"].as_u64().expect("right grams") > 0);
}

#[test]
fn format_inline_preview_does_not_mutate_disk() {
    let project = TestProject::new("format-preview");
    let disk = valid_program("Disk");
    project.write("src/main.st", &disk);

    let payload = format_payload(
        &project.root,
        "src/main.st",
        Some(
            "PROGRAM Main\nVAR\nCounter:INT;\nEND_VAR\nCounter:=Counter+1;\nEND_PROGRAM\n"
                .to_string(),
        ),
    )
    .expect("format preview");

    assert_eq!(payload["path"], "src/main.st");
    assert_eq!(payload["changed"], true);
    assert_eq!(project.read("src/main.st"), disk);
}

#[test]
fn format_without_content_reads_disk_and_reports_unchanged() {
    let project = TestProject::new("format-disk");
    let formatted = "PROGRAM Main\nEND_PROGRAM\n";
    project.write("src/main.st", formatted);

    let payload = format_payload(&project.root, "src/main.st", None).expect("format disk source");

    assert_eq!(payload["content"], formatted);
    assert_eq!(payload["changed"], false);
    assert_eq!(
        Path::new(payload["file"].as_str().expect("absolute file")),
        project.root.join("src/main.st")
    );
}

#[test]
fn format_missing_file_fails_without_creating_it() {
    let project = TestProject::new("format-missing");

    let error =
        format_payload(&project.root, "src/missing.st", None).expect_err("missing file must fail");

    assert!(
        error.to_string().contains("source file not found"),
        "{error:#}"
    );
    assert!(!project.root.join("src/missing.st").exists());
}

#[test]
fn format_rejects_parent_escape() {
    let project = TestProject::new("format-escape");

    let error = format_payload(
        &project.root,
        "../outside.st",
        Some(valid_program("Outside")),
    )
    .expect_err("parent escape must fail");

    assert!(
        error.to_string().contains("format structured text"),
        "{error:#}"
    );
}

#[test]
fn project_info_reports_absent_optional_files_without_false_availability() {
    let project = TestProject::new("project-info-absent");
    project.write("src/main.st", &valid_program("Main"));

    let payload = project_info_payload(&project.root, None).expect("inspect project");

    assert_eq!(payload["version"], 1);
    assert_eq!(payload["sourceCount"], 1);
    assert_eq!(payload["sources"], json!(["src/main.st"]));
    assert_eq!(payload["runtime"]["available"], false);
    assert_eq!(payload["io"]["available"], false);
    for key in [
        "runtimeToml",
        "ioToml",
        "simulationToml",
        "trustLspToml",
        "programStbc",
    ] {
        assert_eq!(payload["files"][key]["exists"], false, "file key: {key}");
    }
}

#[test]
fn project_info_reports_malformed_runtime_and_io_without_failing_orientation() {
    let project = TestProject::new("project-info-parse-errors");
    project.write("src/main.st", &valid_program("Main"));
    project.write("runtime.toml", "not = [valid");
    project.write("io.toml", "[io\n");

    let payload = project_info_payload(&project.root, None).expect("inspect project");

    assert_eq!(payload["runtime"]["available"], true);
    assert!(payload["runtime"]["parseError"].is_string(), "{payload:#}");
    assert_eq!(payload["io"]["available"], true);
    assert!(payload["io"]["parseError"].is_string(), "{payload:#}");
    assert_eq!(payload["files"]["runtimeToml"]["exists"], true);
    assert_eq!(payload["files"]["ioToml"]["exists"], true);
}

#[test]
fn project_info_exposes_vendor_profile_and_manifest_presence() {
    let project = TestProject::new("project-info-vendor");
    project.write("src/main.st", &valid_program("Main"));
    project.write(
        "trust-lsp.toml",
        "[project]\nvendor_profile = \"codesys\"\n",
    );

    let payload = project_info_payload(&project.root, None).expect("inspect project");

    assert_eq!(payload["lsp"]["vendorProfile"], "codesys");
    assert_eq!(payload["files"]["trustLspToml"]["exists"], true);
    assert_eq!(
        Path::new(
            payload["lsp"]["manifestPath"]
                .as_str()
                .expect("manifest path")
        ),
        project.root.join("trust-lsp.toml")
    );
}

#[test]
fn project_info_runtime_summary_redacts_control_token_value() {
    let project = TestProject::new("project-info-runtime");
    project.write("src/main.st", &valid_program("Main"));
    project.write(
        "runtime.toml",
        r#"[bundle]
version = 1

[resource]
name = "Res"
cycle_interval_ms = 100

[runtime.control]
endpoint = "tcp://127.0.0.1:9001"
auth_token = "never-echo-this-token"

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
"#,
    );

    let payload = project_info_payload(&project.root, None).expect("inspect project");
    let encoded = serde_json::to_string(&payload).expect("serialize payload");

    assert_eq!(payload["runtime"]["available"], true);
    assert_eq!(
        payload["runtime"]["controlEndpoint"],
        "tcp://127.0.0.1:9001"
    );
    assert_eq!(payload["runtime"]["hasControlToken"], true);
    assert!(!encoded.contains("never-echo-this-token"), "{encoded}");
}

#[test]
fn project_info_io_summary_reports_driver_and_safe_state_counts() {
    let project = TestProject::new("project-info-io");
    project.write("src/main.st", &valid_program("Main"));
    project.write(
        "io.toml",
        r#"[io]
driver = "simulated"

[io.params]

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
"#,
    );

    let payload = project_info_payload(&project.root, None).expect("inspect project");

    assert_eq!(payload["io"]["available"], true);
    assert_eq!(payload["io"]["driverCount"], 1);
    assert_eq!(payload["io"]["drivers"], json!(["simulated"]));
    assert_eq!(payload["io"]["safeStateCount"], 1);
}

#[test]
fn project_info_program_presence_tracks_real_artifact() {
    let project = TestProject::new("project-info-program");
    project.write("src/main.st", &valid_program("Main"));
    project.write("program.stbc", "not-real-bytecode");

    let payload = project_info_payload(&project.root, None).expect("inspect project");

    assert_eq!(payload["files"]["programStbc"]["exists"], true);
    assert_eq!(
        Path::new(
            payload["files"]["programStbc"]["path"]
                .as_str()
                .expect("program path")
        ),
        project.root.join("program.stbc")
    );
}

#[test]
fn project_info_source_listing_is_deterministic_and_project_relative() {
    let project = TestProject::new("project-info-source-order");
    project.write("src/z.st", &valid_program("Zed"));
    project.write("src/a/nested.POU", &valid_program("Alpha"));
    project.write("src/middle.St", &valid_program("Middle"));

    let first = project_info_payload(&project.root, None).expect("first inspection");
    let second = project_info_payload(&project.root, None).expect("second inspection");

    assert_eq!(first["sources"], second["sources"]);
    assert_eq!(
        first["sources"],
        json!(["src/a/nested.POU", "src/middle.St", "src/z.st"])
    );
}

#[test]
fn project_info_honors_explicit_source_root() {
    let project = TestProject::new("project-info-source-override");
    project.write("src/ignored.st", &valid_program("Ignored"));
    project.write("selected/main.st", &valid_program("Selected"));

    let payload =
        project_info_payload(&project.root, Some(project.root.join("selected").as_path()))
            .expect("inspect selected source root");

    assert_eq!(payload["sourceCount"], 1);
    assert_eq!(payload["sources"], json!(["selected/main.st"]));
    assert_eq!(
        Path::new(payload["sourcesRoot"].as_str().expect("source root")),
        project.root.join("selected")
    );
}
