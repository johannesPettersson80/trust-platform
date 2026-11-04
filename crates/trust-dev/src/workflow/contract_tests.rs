use super::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn issue_files(payload: &Value) -> Vec<String> {
    payload["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .map(|issue| {
            issue["file"]
                .as_str()
                .expect("issue file")
                .replace('\\', "/")
        })
        .collect()
}

#[test]
fn diagnostics_rejects_content_without_path() {
    let project = TestProject::new("diagnostics-content-path");

    let error = diagnostics_payload(
        &project.root,
        &project.root,
        None,
        None,
        Some(valid_program("Inline")),
        None,
    )
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
        &project.root,
        None,
        Some("src/main.st"),
        Some(invalid_program("Inline", "Missing")),
        None,
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

    let payload = diagnostics_payload(&project.root, &project.root, None, None, None, None)
        .expect("analyze project sources");

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
        &project.root,
        Some(project.root.join("selected").as_path()),
        None,
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

    let payload = diagnostics_payload(&project.root, &project.root, None, None, None, None)
        .expect("analyze project sources");
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

    let payload = diagnostics_payload(&project.root, &project.root, None, None, None, None)
        .expect("analyze project sources");

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
    let finding = trust_lsp::Finding {
        path: "src/main.st".to_string(),
        file: PathBuf::from("/project/src/main.st"),
        line: 3,
        column: 5,
        end_line: 3,
        end_column: 10,
        start: 20,
        end: 25,
        severity: "error",
        code: Some("E-DEMO".to_string()),
        message: "primary".to_string(),
        related: vec![trust_lsp::RelatedFinding {
            line: 1,
            column: 2,
            end_line: 1,
            end_column: 4,
            start: 1,
            end: 3,
            message: "origin".to_string(),
        }],
    };

    let projected = serde_json::to_value(map_finding(finding)).expect("serialize issue");

    assert_eq!(
        projected,
        json!({
            "path": "src/main.st",
            "file": Path::new("/project/src/main.st").display().to_string(),
            "line": 3,
            "column": 5,
            "endLine": 3,
            "endColumn": 10,
            "span": {"start": 20, "end": 25},
            "severity": "error",
            "message": "primary",
            "code": "E-DEMO",
            "source": "trust-lsp",
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
    let finding = trust_lsp::Finding {
        path: "main.st".to_string(),
        file: PathBuf::from("/main.st"),
        line: 1,
        column: 1,
        end_line: 1,
        end_column: 2,
        start: 0,
        end: 1,
        severity: "warning",
        code: Some("   ".to_string()),
        message: "warning".to_string(),
        related: Vec::new(),
    };

    let projected = serde_json::to_value(map_finding(finding)).expect("serialize issue");

    assert!(projected.get("code").is_none(), "{projected:#}");
    assert!(projected.get("related").is_none(), "{projected:#}");
    assert_eq!(projected["severity"], "warning");
}

/// Issue #236: the agent measured a CODESYS project under the IEC rules, because its diagnostics
/// came from an engine that never received the vendor profile. Catches that route: specification
/// 14's `codesys` profile (IEC_DEVIATIONS DEV-037) makes a narrowing store a warning, as
/// `trust-lsp check` reports it.
#[test]
fn diagnostics_follow_the_projects_vendor_profile_as_trust_lsp_check_does() {
    let project = TestProject::new("diagnostics-codesys");
    project.write(
        "trust-lsp.toml",
        "[project]\ninclude_paths = [\"src\"]\nvendor_profile = \"codesys\"\n",
    );
    project.write(
        "src/main.st",
        "PROGRAM Main\nVAR\n    s : SINT;\n    b : BYTE;\nEND_VAR\nb := s;\nEND_PROGRAM\n",
    );

    let payload = diagnostics_payload(&project.root, &project.root, None, None, None, None)
        .expect("analyze project");

    let narrowing: Vec<&Value> = payload["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .filter(|issue| issue["code"] == "E203")
        .collect();
    assert_eq!(narrowing.len(), 1, "{payload:#}");
    assert_eq!(narrowing[0]["severity"], "warning", "{payload:#}");
    assert_eq!(narrowing[0]["message"], "cannot assign 'SINT' to 'BYTE'");
    assert_eq!(payload["errors"], 0, "{payload:#}");
}

/// Specification 20 §3 and §7: a library that the project's configuration names outside the agent
/// workspace is analyzed, so that its function block resolves in the project, but its own findings
/// are not listed. Catches a route that leaves the library out (an undefined type in the project)
/// and one that lists findings of files outside the workspace.
#[test]
fn a_library_outside_the_workspace_is_analyzed_but_not_listed() {
    let project = TestProject::new("diagnostics-outside-library");
    let library = TestProject::new("diagnostics-outside-library-shared");
    library.write(
        "fb_lib.st",
        "FUNCTION_BLOCK fbLib\nVAR_INPUT\n    x : INT;\nEND_VAR\nVAR\n    v : INT;\nEND_VAR\nv := MissingInLibrary;\nEND_FUNCTION_BLOCK\n",
    );
    project.write(
        "trust-lsp.toml",
        &format!(
            "[project]\ninclude_paths = [\"src\"]\nlibrary_paths = [{:?}]\n",
            library.root.display().to_string()
        ),
    );
    project.write(
        "src/main.st",
        "PROGRAM Main\nVAR\n    fb : fbLib;\nEND_VAR\nfb(x := 1);\nEND_PROGRAM\n",
    );

    let payload = diagnostics_payload(&project.root, &project.root, None, None, None, None)
        .expect("analyze project");

    let files = issue_files(&payload);
    assert!(
        files.iter().all(|file| !file.ends_with("fb_lib.st")),
        "a file outside the workspace is not listed: {payload:#}"
    );
    assert!(
        !payload["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .any(|issue| issue["message"].as_str().unwrap_or("").contains("fbLib")),
        "the library's function block resolves: {payload:#}"
    );
}

/// The listed issues as (path, line, code).
fn listed(payload: &Value) -> Vec<(String, u64, String)> {
    payload["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .map(|issue| {
            (
                issue["path"].as_str().expect("issue path").to_owned(),
                issue["line"].as_u64().expect("issue line"),
                issue["code"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// Specification 20 §7: `page` and `page_size` cut a window of the sorted issues (#236: an
/// unpaged response of 13.7 MB exceeded real clients' frame limits), while `errors`, `warnings`
/// and `totalIssues` count every listed issue. Catches a page cut before the sort, counts over the
/// page alone, a page past the last that repeats it, and paging fields on an unpaged response.
#[test]
fn a_page_lists_its_issues_and_the_counts_cover_all_of_them() {
    let project = TestProject::new("diagnostics-page");
    for name in ["c", "a", "b"] {
        project.write(
            &format!("src/{name}.st"),
            &invalid_program(&format!("P{name}"), "Missing"),
        );
    }
    // Each file holds a program that no configuration runs (W009 on line 1) and an undefined
    // identifier (E101 on line 5): six issues, three errors and three warnings, by file and line.
    let issue =
        |file: &str, line: u64, code: &str| (format!("src/{file}.st"), line, code.to_owned());
    let all = diagnostics_payload(&project.root, &project.root, None, None, None, None)
        .expect("analyze project");
    assert_eq!(
        listed(&all),
        vec![
            issue("a", 1, "W009"),
            issue("a", 5, "E101"),
            issue("b", 1, "W009"),
            issue("b", 5, "E101"),
            issue("c", 1, "W009"),
            issue("c", 5, "E101"),
        ],
        "{all:#}"
    );
    assert_eq!((&all["errors"], &all["warnings"]), (&json!(3), &json!(3)));
    for field in ["page", "pageSize", "totalIssues", "totalPages"] {
        assert!(
            all.get(field).is_none(),
            "unpaged, without {field}: {all:#}"
        );
    }

    let page = |number, size| {
        diagnostics_payload(
            &project.root,
            &project.root,
            None,
            None,
            None,
            Some(IssuePage { number, size }),
        )
        .expect("analyze project")
    };
    // Page 2 of four issues each holds the fifth and the sixth.
    let second = page(2, 4);
    assert_eq!(
        listed(&second),
        vec![issue("c", 1, "W009"), issue("c", 5, "E101")]
    );
    assert_eq!(
        [
            &second["errors"],
            &second["warnings"],
            &second["totalIssues"]
        ],
        [&json!(3), &json!(3), &json!(6)]
    );
    assert_eq!(
        [&second["page"], &second["pageSize"], &second["totalPages"]],
        [&json!(2), &json!(4), &json!(2)]
    );
    // A page past the last is empty, with the same totals.
    let past = page(3, 4);
    assert_eq!(listed(&past), Vec::new());
    assert_eq!(
        [&past["totalIssues"], &past["totalPages"]],
        [&json!(6), &json!(2)]
    );

    // A project without issues has one page, an empty one.
    let clean = TestProject::new("diagnostics-page-clean");
    clean.write(
        "src/main.st",
        "CONFIGURATION Plant\nTASK MainTask (PRIORITY := 1);\nPROGRAM MainInstance WITH MainTask : Main;\nEND_CONFIGURATION\nPROGRAM Main\nEND_PROGRAM\n",
    );
    let empty = diagnostics_payload(
        &clean.root,
        &clean.root,
        None,
        None,
        None,
        Some(IssuePage { number: 1, size: 4 }),
    )
    .expect("analyze project");
    assert_eq!(listed(&empty), Vec::new(), "{empty:#}");
    assert_eq!(
        [&empty["totalIssues"], &empty["totalPages"]],
        [&json!(0), &json!(1)]
    );
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
        "programImage",
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
    project.write("program.tpi", "not-a-real-image");

    let payload = project_info_payload(&project.root, None).expect("inspect project");

    assert_eq!(payload["files"]["programImage"]["exists"], true);
    assert_eq!(
        Path::new(
            payload["files"]["programImage"]["path"]
                .as_str()
                .expect("program path")
        ),
        project.root.join("program.tpi")
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
