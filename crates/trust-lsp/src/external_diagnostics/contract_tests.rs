use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIAGNOSTIC_ID: AtomicU64 = AtomicU64::new(0);

struct DiagnosticProject {
    root: PathBuf,
}

impl DiagnosticProject {
    fn new(label: &str) -> Self {
        let id = NEXT_DIAGNOSTIC_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-external-diagnostic-contract-{}-{label}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create external diagnostic project");
        Self { root }
    }

    fn write(&self, relative: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create external diagnostic fixture parent");
        }
        fs::write(&path, contents).expect("write external diagnostic fixture");
        path
    }

    fn config(&self, paths: &[&str]) -> ProjectConfig {
        let external_paths = paths
            .iter()
            .map(|path| format!("{path:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        ProjectConfig::from_contents(
            &self.root,
            Some(self.root.join("trust-lsp.toml")),
            &format!(
                r#"
[diagnostics]
external_paths = [{external_paths}]
"#
            ),
        )
    }

    fn uri(&self, relative: &str) -> Url {
        path_to_uri(&self.root.join(relative)).expect("fixture file URI")
    }
}

impl Drop for DiagnosticProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

fn base_entry(target: &str) -> String {
    format!(
        r#"{{
  "path": {},
  "range": {{
    "start": {{ "line": 1, "character": 2 }},
    "end": {{ "line": 3, "character": 4 }}
  }},
  "message": "external finding"
}}"#,
        serde_json::to_string(target).expect("serialize diagnostic path"),
    )
}

#[test]
fn top_level_list_is_accepted() {
    let project = DiagnosticProject::new("list");
    project.write("diagnostics.json", format!("[{}]", base_entry("main.st")));
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "external finding");
}

#[test]
fn wrapper_object_is_accepted() {
    let project = DiagnosticProject::new("wrapper");
    project.write(
        "diagnostics.json",
        format!(r#"{{"diagnostics":[{}]}}"#, base_entry("main.st")),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn malformed_document_contributes_nothing() {
    let project = DiagnosticProject::new("malformed");
    project.write("diagnostics.json", "[{not-json");
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn one_schema_invalid_entry_rejects_whole_document_atomically() {
    let project = DiagnosticProject::new("schema-atomic");
    project.write(
        "diagnostics.json",
        format!(
            r#"[{},{{"path":"main.st","message":"missing range"}}]"#,
            base_entry("main.st")
        ),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn unreadable_and_missing_documents_contribute_nothing() {
    let project = DiagnosticProject::new("missing");
    let diagnostics =
        collect_external_diagnostics(&project.config(&["missing.json"]), &project.uri("main.st"));
    assert!(diagnostics.is_empty());
}

#[test]
fn relative_entry_path_resolves_against_project_root() {
    let project = DiagnosticProject::new("relative-path");
    project.write(
        "diagnostics.json",
        format!("[{}]", base_entry("src/main.st")),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("src/main.st"),
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn absolute_entry_path_is_preserved() {
    let project = DiagnosticProject::new("absolute-path");
    let target = project.root.join("src/main.st");
    project.write(
        "diagnostics.json",
        format!("[{}]", base_entry(&target.to_string_lossy())),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("src/main.st"),
    );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn explicit_uri_takes_precedence_over_path() {
    let project = DiagnosticProject::new("uri-precedence");
    let selected = project.uri("selected.st");
    project.write(
        "diagnostics.json",
        format!(
            r#"[{{
  "uri": {:?},
  "path": "other.st",
  "range": {{"start":{{"line":0,"character":0}},"end":{{"line":0,"character":1}}}},
  "message": "selected"
}}]"#,
            selected.as_str()
        ),
    );
    let config = project.config(&["diagnostics.json"]);
    assert_eq!(collect_external_diagnostics(&config, &selected).len(), 1);
    assert!(collect_external_diagnostics(&config, &project.uri("other.st")).is_empty());
}

#[test]
fn invalid_explicit_uri_does_not_fall_back_to_path() {
    let project = DiagnosticProject::new("invalid-uri");
    project.write(
        "diagnostics.json",
        r#"[{
  "uri": "not a URI",
  "path": "main.st",
  "range": {"start":{"line":0,"character":0},"end":{"line":0,"character":1}},
  "message": "invalid target"
}]"#,
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn entries_for_other_documents_are_filtered() {
    let project = DiagnosticProject::new("document-filter");
    project.write("diagnostics.json", format!("[{}]", base_entry("other.st")));
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn missing_uri_and_path_make_entry_inapplicable() {
    let project = DiagnosticProject::new("missing-target");
    project.write(
        "diagnostics.json",
        r#"[{
  "range": {"start":{"line":0,"character":0},"end":{"line":0,"character":1}},
  "message": "untargeted"
}]"#,
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn lsp_range_is_preserved_exactly_and_zero_based() {
    let project = DiagnosticProject::new("range");
    project.write("diagnostics.json", format!("[{}]", base_entry("main.st")));
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(diagnostics[0].range.start, Position::new(1, 2));
    assert_eq!(diagnostics[0].range.end, Position::new(3, 4));
}

#[test]
fn absent_severity_defaults_to_warning() {
    let project = DiagnosticProject::new("severity-default");
    project.write("diagnostics.json", format!("[{}]", base_entry("main.st")));
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::WARNING));
}

#[test]
fn string_severity_vocabulary_is_trimmed_and_case_insensitive() {
    for (value, expected) in [
        (" ERROR ", DiagnosticSeverity::ERROR),
        ("Warning", DiagnosticSeverity::WARNING),
        ("INFO", DiagnosticSeverity::INFORMATION),
        ("information", DiagnosticSeverity::INFORMATION),
        ("Hint", DiagnosticSeverity::HINT),
    ] {
        let severity = ExternalSeverity::String(value.to_string());
        assert_eq!(severity.to_lsp(), Some(expected), "{value}");
    }
}

#[test]
fn numeric_severity_vocabulary_matches_lsp_values() {
    for (value, expected) in [
        (1, DiagnosticSeverity::ERROR),
        (2, DiagnosticSeverity::WARNING),
        (3, DiagnosticSeverity::INFORMATION),
        (4, DiagnosticSeverity::HINT),
    ] {
        assert_eq!(ExternalSeverity::Number(value).to_lsp(), Some(expected));
    }
    assert_eq!(ExternalSeverity::Number(0).to_lsp(), None);
    assert_eq!(ExternalSeverity::Number(5).to_lsp(), None);
}

#[test]
fn invalid_severity_defaults_to_warning_at_ingestion_boundary() {
    let project = DiagnosticProject::new("severity-invalid");
    project.write(
        "diagnostics.json",
        format!(
            "[{}]",
            base_entry("main.st").replace(
                r#""message": "external finding""#,
                r#""severity":"fatal","message":"external finding""#
            )
        ),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::WARNING));
}

#[test]
fn source_defaults_to_external_and_code_remains_string() {
    let project = DiagnosticProject::new("source-code");
    project.write(
        "diagnostics.json",
        format!(
            "[{}]",
            base_entry("main.st").replace(
                r#""message": "external finding""#,
                r#""code":"0042","message":"external finding""#
            )
        ),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(diagnostics[0].source.as_deref(), Some("external"));
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String("0042".to_string()))
    );
}

#[test]
fn explicit_source_is_preserved() {
    let project = DiagnosticProject::new("source-explicit");
    project.write(
        "diagnostics.json",
        format!(
            "[{}]",
            base_entry("main.st").replace(
                r#""message": "external finding""#,
                r#""source":"safety-lint","message":"external finding""#
            )
        ),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(diagnostics[0].source.as_deref(), Some("safety-lint"));
}

#[test]
fn fix_data_preserves_title_text_and_explicit_range() {
    let project = DiagnosticProject::new("fix");
    project.write(
        "diagnostics.json",
        r#"[{
  "path": "main.st",
  "range": {"start":{"line":1,"character":2},"end":{"line":1,"character":5}},
  "message": "replace token",
  "fix": {
    "title": "Use TRUE",
    "range": {"start":{"line":1,"character":2},"end":{"line":1,"character":7}},
    "new_text": "TRUE"
  }
}]"#,
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    let fix = &diagnostics[0].data.as_ref().expect("fix data")["externalFix"];
    assert_eq!(fix["title"], "Use TRUE");
    assert_eq!(fix["newText"], "TRUE");
    assert_eq!(fix["range"]["start"]["line"], 1);
    assert_eq!(fix["range"]["end"]["character"], 7);
}

#[test]
fn fix_data_allows_omitted_title_and_range() {
    let project = DiagnosticProject::new("fix-minimal");
    project.write(
        "diagnostics.json",
        r#"[{
  "path": "main.st",
  "range": {"start":{"line":0,"character":0},"end":{"line":0,"character":1}},
  "message": "replace token",
  "fix": {"new_text": "X"}
}]"#,
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["diagnostics.json"]),
        &project.uri("main.st"),
    );
    let fix = &diagnostics[0].data.as_ref().expect("fix data")["externalFix"];
    assert!(fix["title"].is_null());
    assert!(fix["range"].is_null());
    assert_eq!(fix["newText"], "X");
}

#[test]
fn files_and_entries_preserve_declaration_order() {
    let project = DiagnosticProject::new("ordering");
    project.write(
        "first.json",
        format!(
            "[{},{}]",
            base_entry("main.st").replace("external finding", "first-a"),
            base_entry("main.st").replace("external finding", "first-b")
        ),
    );
    project.write(
        "second.json",
        format!(
            "[{}]",
            base_entry("main.st").replace("external finding", "second")
        ),
    );
    let diagnostics = collect_external_diagnostics(
        &project.config(&["first.json", "second.json"]),
        &project.uri("main.st"),
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec!["first-a", "first-b", "second"]
    );
}
