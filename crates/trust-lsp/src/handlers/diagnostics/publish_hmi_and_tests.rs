#[cfg(test)]
fn collect_hmi_toml_diagnostics_for_root(
    root: &Path,
    current_file: &Path,
    content: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = collect_hmi_toml_parse_diagnostics(content);
    if !diagnostics.is_empty() {
        return diagnostics;
    }
    diagnostics.extend(collect_hmi_toml_semantic_diagnostics(
        root,
        current_file,
        content,
    ));
    diagnostics
}

fn collect_config_diagnostics(
    state: &ServerState,
    uri: &Url,
    content: &str,
    root_hint: Option<&Url>,
) -> Vec<Diagnostic> {
    let root = root_hint
        .and_then(uri_to_path)
        .or_else(|| config_root_for_uri(state, uri))
        .unwrap_or_else(|| PathBuf::from("."));
    let config_path = uri_to_path(uri);
    let config = ProjectConfig::from_contents(&root, config_path, content);
    let mut diagnostics = Vec::new();
    for issue in &config.dependency_resolution_issues {
        let range = find_name_range(content, issue.dependency.as_str());
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(issue.code.to_string())),
            source: Some("truST".to_string()),
            message: issue.message.clone(),
            ..Default::default()
        });
    }
    for issue in library_dependency_issues(&config) {
        let target = issue
            .dependency
            .as_deref()
            .unwrap_or(issue.subject.as_str());
        let range = find_name_range(content, target);
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(issue.code.to_string())),
            source: Some("truST".to_string()),
            message: issue.message,
            ..Default::default()
        });
    }
    diagnostics
}

fn config_root_for_uri(state: &ServerState, uri: &Url) -> Option<PathBuf> {
    state
        .workspace_config_for_uri(uri)
        .map(|config| config.root)
        .or_else(|| uri_to_path(uri).and_then(|path| path.parent().map(Path::to_path_buf)))
}

fn find_name_range(content: &str, name: &str) -> Range {
    if name.is_empty() {
        return fallback_range(content);
    }
    let quoted = format!("\"{name}\"");
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(pos) = line.find(&quoted) {
            let start = pos + 1;
            let end = start + name.len();
            return Range {
                start: tower_lsp::lsp_types::Position::new(line_idx as u32, start as u32),
                end: tower_lsp::lsp_types::Position::new(line_idx as u32, end as u32),
            };
        }
        if let Some(pos) = line.find(name) {
            let end = pos + name.len();
            return Range {
                start: tower_lsp::lsp_types::Position::new(line_idx as u32, pos as u32),
                end: tower_lsp::lsp_types::Position::new(line_idx as u32, end as u32),
            };
        }
    }
    fallback_range(content)
}

fn fallback_range(content: &str) -> Range {
    let end = content
        .lines()
        .next()
        .map(|line| line.len() as u32)
        .unwrap_or(0);
    Range {
        start: tower_lsp::lsp_types::Position::new(0, 0),
        end: tower_lsp::lsp_types::Position::new(0, end),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_hmi_toml_diagnostics_for_root, diagnostic_code, top_ranked_suggestions,
        LearnerContext, HMI_DIAG_INVALID_PROPERTIES, HMI_DIAG_UNKNOWN_BIND,
    };
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(path, content).expect("write file");
    }

    #[test]
    fn suggestion_ranking_prefers_closest_match() {
        let context = LearnerContext {
            value_candidates: vec![
                "speedValue".to_string(),
                "seedValue".to_string(),
                "setpoint".to_string(),
            ],
            type_candidates: Vec::new(),
        };
        let suggestions = top_ranked_suggestions("speadValue", &context.value_candidates);
        assert_eq!(
            suggestions.first().map(String::as_str),
            Some("speedValue"),
            "closest typo fix should rank first"
        );
    }

    #[test]
    fn suggestion_ranking_suppresses_low_confidence_noise() {
        let context = LearnerContext {
            value_candidates: vec![
                "temperature".to_string(),
                "counter".to_string(),
                "runtimeTicks".to_string(),
            ],
            type_candidates: Vec::new(),
        };
        let suggestions = top_ranked_suggestions("zzzzzzz", &context.value_candidates);
        assert!(
            suggestions.is_empty(),
            "unrelated names should not produce misleading suggestions"
        );
    }

    #[test]
    fn hmi_toml_diagnostics_report_unknown_bind_with_near_match_hint() {
        let root = temp_dir("trust-lsp-hmi-diag-unknown-bind");
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL;
END_VAR
END_PROGRAM
"#,
        );
        let page_path = root.join("hmi/overview.toml");
        let page = r#"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "gauge"
bind = "Main.spead"
"#;
        write_file(&page_path, page);

        let diagnostics = collect_hmi_toml_diagnostics_for_root(&root, &page_path, page);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic_code(diagnostic).as_deref() == Some(HMI_DIAG_UNKNOWN_BIND)
                && diagnostic.message.contains("Main.speed")
        }));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_toml_diagnostics_report_type_widget_and_property_issues() {
        let root = temp_dir("trust-lsp-hmi-diag-invalid-widget");
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR_OUTPUT
    run : BOOL;
    speed : REAL;
END_VAR
END_PROGRAM
"#,
        );
        let page_path = root.join("hmi/overview.toml");
        let page = r##"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "gauge"
bind = "Main.run"

[[section.widget]]
type = "rocket"
bind = "Main.speed"

[[section.widget]]
type = "bar"
bind = "Main.speed"
on_color = "#22c55e"

[[section.widget]]
type = "indicator"
bind = "Main.run"
min = 10
max = 1
"##;
        write_file(&page_path, page);

        let diagnostics = collect_hmi_toml_diagnostics_for_root(&root, &page_path, page);
        let codes = diagnostics
            .iter()
            .filter_map(diagnostic_code)
            .collect::<Vec<_>>();
        assert!(codes.iter().any(|code| code == "HMI_BIND_TYPE_MISMATCH"));
        assert!(codes.iter().any(|code| code == "HMI_UNKNOWN_WIDGET_KIND"));
        assert!(codes.iter().any(|code| code == HMI_DIAG_INVALID_PROPERTIES));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_toml_diagnostics_avoid_false_positives_for_valid_page() {
        let root = temp_dir("trust-lsp-hmi-diag-valid");
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR_OUTPUT
    run : BOOL;
    speed : REAL;
END_VAR
END_PROGRAM
"#,
        );
        let page_path = root.join("hmi/overview.toml");
        let page = r##"
title = "Overview"
kind = "dashboard"

[[section]]
title = "Main"

[[section.widget]]
type = "indicator"
bind = "Main.run"
on_color = "#22c55e"
off_color = "#94a3b8"

[[section.widget]]
type = "gauge"
bind = "Main.speed"
min = 0
max = 100
"##;
        write_file(&page_path, page);

        let diagnostics = collect_hmi_toml_diagnostics_for_root(&root, &page_path, page);
        assert!(
            diagnostics.is_empty(),
            "valid descriptor should not produce diagnostics: {diagnostics:#?}"
        );

        std::fs::remove_dir_all(root).ok();
    }
}
