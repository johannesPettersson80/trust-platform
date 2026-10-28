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
    for issue in crate::config::configuration_issues(content) {
        diagnostics.push(Diagnostic {
            range: find_name_range(content, &issue.subject),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(issue.code.to_string())),
            source: Some("truST".to_string()),
            message: issue.message,
            ..Default::default()
        });
    }
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
    use super::{top_ranked_suggestions, LearnerContext};

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
}
