fn apply_diagnostic_filters(state: &ServerState, uri: &Url, diagnostics: &mut Vec<Diagnostic>) {
    let Some(config) = state.workspace_config_for_uri(uri) else {
        return;
    };
    let settings = config.diagnostics;
    diagnostics.retain(|diagnostic| diagnostic_allowed(&settings, diagnostic));
}

fn apply_diagnostic_overrides(state: &ServerState, uri: &Url, diagnostics: &mut [Diagnostic]) {
    let Some(config) = state.workspace_config_for_uri(uri) else {
        return;
    };
    let overrides = &config.diagnostics.severity_overrides;
    if overrides.is_empty() {
        return;
    }
    for diagnostic in diagnostics {
        let Some(code) = diagnostic_code(diagnostic) else {
            continue;
        };
        if let Some(severity) = overrides.get(&code) {
            diagnostic.severity = Some(*severity);
        }
    }
}

fn diagnostic_allowed(settings: &DiagnosticSettings, diagnostic: &Diagnostic) -> bool {
    let Some(code) = diagnostic_code(diagnostic) else {
        return true;
    };
    match code.as_str() {
        "W001" | "W002" | "W009" => settings.warn_unused,
        "W003" => settings.warn_unreachable,
        "W004" => settings.warn_missing_else,
        "W005" => settings.warn_implicit_conversion,
        "W006" => settings.warn_shadowed,
        "W007" => settings.warn_deprecated,
        "W008" => settings.warn_complexity,
        "W010" | "W011" => settings.warn_nondeterminism,
        "W013" | "W014" => settings.warn_numeric_hazards,
        _ => true,
    }
}

#[derive(Default, Debug, Clone)]
struct LearnerContext {
    value_candidates: Vec<String>,
    type_candidates: Vec<String>,
}

fn build_learner_context(state: &ServerState, file_id: FileId) -> LearnerContext {
    state.with_database(|db| {
        let symbols = db.file_symbols_with_project(file_id);
        let mut value_map = BTreeMap::<String, String>::new();
        let mut type_map = BTreeMap::<String, String>::new();

        for symbol in symbols.iter() {
            if is_value_suggestion_kind(&symbol.kind) {
                let name = symbol.name.to_string();
                value_map.entry(name.to_ascii_uppercase()).or_insert(name);
            }
            if symbol.is_type() {
                let name = symbol.name.to_string();
                type_map.entry(name.to_ascii_uppercase()).or_insert(name);
            }
        }

        for builtin in BUILTIN_TYPE_NAMES {
            let name = builtin.to_string();
            type_map
                .entry(name.to_ascii_uppercase())
                .or_insert_with(|| name);
        }

        LearnerContext {
            value_candidates: value_map.into_values().collect(),
            type_candidates: type_map.into_values().collect(),
        }
    })
}

const BUILTIN_TYPE_NAMES: &[&str] = &[
    "BOOL", "BYTE", "WORD", "DWORD", "LWORD", "SINT", "INT", "DINT", "LINT", "USINT", "UINT",
    "UDINT", "ULINT", "REAL", "LREAL", "TIME", "LTIME", "DATE", "LDATE", "TOD", "LTOD", "DT",
    "LDT", "STRING", "WSTRING", "CHAR", "WCHAR", "POINTER",
];

const HMI_DIAG_UNKNOWN_BIND: &str = "HMI_BIND_UNKNOWN_PATH";
const HMI_DIAG_INVALID_PROPERTIES: &str = "HMI_INVALID_WIDGET_PROPERTIES";

fn is_value_suggestion_kind(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Variable { .. }
            | SymbolKind::Constant
            | SymbolKind::Parameter { .. }
            | SymbolKind::EnumValue { .. }
            | SymbolKind::ProgramInstance
    )
}

fn attach_explainers(
    state: &ServerState,
    uri: &Url,
    content: &str,
    learner_context: Option<&LearnerContext>,
    diagnostics: &mut [Diagnostic],
) {
    for diagnostic in diagnostics {
        let Some(code) = diagnostic_code(diagnostic) else {
            continue;
        };
        let mut data = match diagnostic.data.take() {
            Some(Value::Object(map)) => map,
            _ => Map::new(),
        };

        if let Some(explainer) = diagnostic_explainer(&code, &diagnostic.message) {
            if diagnostic.code_description.is_none() {
                if let Some(href) = spec_url(state, explainer.spec_path) {
                    diagnostic.code_description = Some(CodeDescription { href });
                }
            }
            data.insert(
                "explain".to_string(),
                json!({
                    "iec": explainer.iec_ref,
                    "spec": explainer.spec_path,
                }),
            );
        }

        let mut hints = Vec::new();
        let mut did_you_mean = Vec::new();

        if let Some(context) = learner_context {
            let suggestions = did_you_mean_suggestions(&code, &diagnostic.message, context);
            if !suggestions.is_empty() {
                hints.push(format!(
                    "Did you mean {}?",
                    format_suggestion_list(&suggestions)
                ));
                did_you_mean = suggestions;
            }
        }

        hints.extend(syntax_habit_hints(&code, diagnostic, content));

        if let Some(hint) = conversion_guidance_hint(&code, &diagnostic.message) {
            hints.push(hint);
        }

        dedupe_preserve_order(&mut hints);
        for hint in &hints {
            push_related_hint(diagnostic, uri, hint);
        }
        if !hints.is_empty() {
            data.insert("hints".to_string(), json!(hints));
        }
        if !did_you_mean.is_empty() {
            data.insert("didYouMean".to_string(), json!(did_you_mean));
        }

        if !data.is_empty() {
            diagnostic.data = Some(Value::Object(data));
        }
    }
}

fn push_related_hint(diagnostic: &mut Diagnostic, uri: &Url, hint: &str) {
    let message = format!("Hint: {hint}");
    if diagnostic
        .related_information
        .as_ref()
        .is_some_and(|related| {
            related
                .iter()
                .any(|info| info.message.eq_ignore_ascii_case(&message))
        })
    {
        return;
    }
    diagnostic
        .related_information
        .get_or_insert_with(Vec::new)
        .push(DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: diagnostic.range,
            },
            message,
        });
}

#[cfg(test)]
mod collection_filter_tests {
    use super::*;
    use tower_lsp::lsp_types::{
        Diagnostic, DiagnosticSeverity as LspDiagnosticSeverity, NumberOrString, Range,
    };

    fn warning(code: &str) -> Diagnostic {
        Diagnostic {
            range: Range::default(),
            severity: Some(LspDiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(code.to_string())),
            ..Diagnostic::default()
        }
    }

    #[test]
    fn numeric_hazard_filter_controls_numeric_warning_codes() {
        let mut settings = DiagnosticSettings {
            warn_numeric_hazards: false,
            ..DiagnosticSettings::default()
        };

        assert!(!diagnostic_allowed(&settings, &warning("W013")));
        assert!(!diagnostic_allowed(&settings, &warning("W014")));

        settings.warn_numeric_hazards = true;
        assert!(diagnostic_allowed(&settings, &warning("W013")));
        assert!(diagnostic_allowed(&settings, &warning("W014")));
    }
}

fn did_you_mean_suggestions(code: &str, message: &str, context: &LearnerContext) -> Vec<String> {
    match code {
        "E101" => {
            let Some(query) = extract_quoted_after_prefix(message, "undefined identifier '") else {
                return Vec::new();
            };
            top_ranked_suggestions(&query, &context.value_candidates)
        }
        "E102" => {
            let query = extract_quoted_after_prefix(message, "cannot resolve type '")
                .or_else(|| extract_quoted_after_prefix(message, "cannot resolve interface '"));
            let Some(query) = query else {
                return Vec::new();
            };
            top_ranked_suggestions(&query, &context.type_candidates)
        }
        _ => Vec::new(),
    }
}

fn extract_quoted_after_prefix(message: &str, prefix: &str) -> Option<String> {
    let tail = message.strip_prefix(prefix)?;
    let end = tail.find('\'')?;
    let value = tail[..end].trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn format_suggestion_list(suggestions: &[String]) -> String {
    match suggestions {
        [] => String::new(),
        [one] => format!("'{one}'"),
        [one, two] => format!("'{one}' or '{two}'"),
        [one, two, three, ..] => format!("'{one}', '{two}', or '{three}'"),
    }
}

fn top_ranked_suggestions(query: &str, candidates: &[String]) -> Vec<String> {
    let normalized_query = normalize_identifier(query);
    if normalized_query.len() < 3 {
        return Vec::new();
    }
    let (min_score, max_distance) = suggestion_thresholds(normalized_query.len());
    let mut seen = std::collections::HashSet::new();
    let mut scored = Vec::new();

    for candidate in candidates {
        let normalized_candidate = normalize_identifier(candidate);
        if normalized_candidate.is_empty() || normalized_candidate == normalized_query {
            continue;
        }
        if !seen.insert(normalized_candidate.clone()) {
            continue;
        }

        let distance = levenshtein_distance(&normalized_query, &normalized_candidate);
        if distance > max_distance {
            continue;
        }
        let score = similarity_score(&normalized_query, &normalized_candidate, distance);
        if score < min_score {
            continue;
        }
        scored.push((score, distance, candidate.clone()));
    }

    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.len().cmp(&b.2.len()))
            .then_with(|| a.2.cmp(&b.2))
    });

    scored
        .into_iter()
        .take(3)
        .map(|(_, _, name)| name)
        .collect()
}

fn suggestion_thresholds(query_len: usize) -> (f32, usize) {
    match query_len {
        0..=2 => (1.0, 0),
        3..=4 => (0.80, 1),
        5..=7 => (0.67, 2),
        8..=12 => (0.60, 3),
        _ => (0.55, 4),
    }
}

fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '.')
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

fn similarity_score(query: &str, candidate: &str, distance: usize) -> f32 {
    let max_len = query.len().max(candidate.len()).max(1) as f32;
    let mut score = 1.0 - (distance as f32 / max_len);
    if candidate.starts_with(query) || query.starts_with(candidate) {
        score += 0.20;
    } else if candidate.contains(query) || query.contains(candidate) {
        score += 0.10;
    }
    if query
        .chars()
        .next()
        .zip(candidate.chars().next())
        .is_some_and(|(a, b)| a == b)
    {
        score += 0.06;
    }
    score.clamp(0.0, 1.0)
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    if a.is_empty() {
        return b.chars().count();
    }
    if b.is_empty() {
        return a.chars().count();
    }

    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr: Vec<usize> = vec![0; b_chars.len() + 1];

    for (i, a_char) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = usize::from(a_char != *b_char);
            let deletion = prev[j + 1] + 1;
            let insertion = curr[j] + 1;
            let substitution = prev[j] + cost;
            curr[j + 1] = deletion.min(insertion).min(substitution);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_chars.len()]
}

fn syntax_habit_hints(code: &str, diagnostic: &Diagnostic, content: &str) -> Vec<String> {
    if !matches!(code, "E001" | "E002" | "E003") {
        return Vec::new();
    }
    let mut hints = Vec::new();
    let snippet = diagnostic_snippet(content, diagnostic).unwrap_or_default();
    let line = content
        .lines()
        .nth(diagnostic.range.start.line as usize)
        .unwrap_or_default();
    let combined = format!("{line} {snippet}");
    let message = diagnostic.message.to_ascii_lowercase();

    if combined.contains("==") {
        hints.push("In Structured Text, use '=' for comparison and ':=' for assignment.".into());
    }
    if combined.contains("&&") {
        hints.push("In Structured Text, use AND instead of &&.".into());
    }
    if combined.contains("||") {
        hints.push("In Structured Text, use OR instead of ||.".into());
    }
    if combined.contains('{') || combined.contains('}') {
        hints.push("Structured Text uses END_* keywords for block endings, not '{' or '}'.".into());
    }

    let plain_equal = snippet.trim() == "="
        || (message.contains(":=") && contains_plain_equal(&combined) && !combined.contains("=="));
    if plain_equal {
        hints.push("In Structured Text, assignments use ':='.".into());
    }

    hints
}

fn diagnostic_snippet(content: &str, diagnostic: &Diagnostic) -> Option<String> {
    let start = position_to_offset(content, diagnostic.range.start)? as usize;
    let end = position_to_offset(content, diagnostic.range.end)? as usize;
    if start >= content.len() {
        return None;
    }
    let end = end.min(content.len());
    if start >= end {
        return None;
    }
    Some(content[start..end].to_string())
}

fn contains_plain_equal(text: &str) -> bool {
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'=' {
            continue;
        }
        let prev = if index > 0 { bytes[index - 1] } else { b'\0' };
        let next = if index + 1 < bytes.len() {
            bytes[index + 1]
        } else {
            b'\0'
        };
        if prev != b':' && prev != b'<' && prev != b'>' && next != b'=' {
            return true;
        }
    }
    false
}

fn conversion_guidance_hint(code: &str, message: &str) -> Option<String> {
    if !matches!(code, "E201" | "E203" | "E207" | "W005") {
        return None;
    }
    let quoted = collect_quoted_segments(message);
    let (source, target) = if message
        .to_ascii_lowercase()
        .starts_with("return type mismatch: expected '")
    {
        if quoted.len() < 2 {
            return None;
        }
        (quoted[1], quoted[0])
    } else if quoted.len() >= 2 {
        (quoted[0], quoted[1])
    } else {
        return None;
    };

    let source = normalize_identifier(source);
    let target = normalize_identifier(target);
    if source.is_empty() || target.is_empty() || source == target {
        return None;
    }
    if !source
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
        || !target
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return None;
    }

    Some(format!(
        "Use an explicit conversion to make intent clear, e.g. `{source}_TO_{target}(<expr>)`."
    ))
}

fn collect_quoted_segments(message: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0usize;
    while let Some(open) = message[start..].find('\'') {
        let open = start + open + 1;
        let Some(close) = message[open..].find('\'') else {
            break;
        };
        let close = open + close;
        if close > open {
            result.push(&message[open..close]);
        }
        start = close + 1;
    }
    result
}

fn dedupe_preserve_order(items: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.to_ascii_lowercase()));
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<String> {
    diagnostic.code.as_ref().map(|code| match code {
        NumberOrString::String(value) => value.clone(),
        NumberOrString::Number(value) => value.to_string(),
    })
}

fn hash_content(content: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn hash_diagnostics(diagnostics: &[Diagnostic]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for diagnostic in diagnostics {
        diagnostic.range.start.line.hash(&mut hasher);
        diagnostic.range.start.character.hash(&mut hasher);
        diagnostic.range.end.line.hash(&mut hasher);
        diagnostic.range.end.character.hash(&mut hasher);
        let severity_key = match diagnostic.severity {
            Some(severity) if severity == DiagnosticSeverity::ERROR => 1,
            Some(severity) if severity == DiagnosticSeverity::WARNING => 2,
            Some(severity) if severity == DiagnosticSeverity::INFORMATION => 3,
            Some(severity) if severity == DiagnosticSeverity::HINT => 4,
            Some(_) => 0,
            None => 0,
        };
        severity_key.hash(&mut hasher);
        diagnostic_code(diagnostic).hash(&mut hasher);
        diagnostic.source.hash(&mut hasher);
        diagnostic.message.hash(&mut hasher);
        if let Some(related) = &diagnostic.related_information {
            related.len().hash(&mut hasher);
            for item in related {
                item.location.range.start.line.hash(&mut hasher);
                item.location.range.start.character.hash(&mut hasher);
                item.location.range.end.line.hash(&mut hasher);
                item.location.range.end.character.hash(&mut hasher);
                item.message.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

struct DiagnosticExplainer {
    iec_ref: &'static str,
    spec_path: &'static str,
}

fn diagnostic_explainer(code: &str, message: &str) -> Option<DiagnosticExplainer> {
    if code == "E202" && is_oop_access_message(message) {
        return Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.6.5; Table 50",
            spec_path: "docs/specs/09-semantic-rules.md",
        });
    }
    match code {
        "E001" | "E002" | "E003" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3",
            spec_path: "docs/specs/06-statements.md",
        }),
        "E101" | "E104" | "E105" | "W001" | "W002" | "W006" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.5.2.2",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "E102" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.2",
            spec_path: "docs/specs/02-data-types.md",
        }),
        "E103" | "E204" | "E205" | "E206" | "E207" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.6.1",
            spec_path: "docs/specs/04-pou-declarations.md",
        }),
        "E106" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.1.2",
            spec_path: "docs/specs/01-lexical-elements.md",
        }),
        "E201" | "E202" | "E203" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3.2",
            spec_path: "docs/specs/05-expressions.md",
        }),
        "E301" | "E302" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3.1",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "E306" | "E307" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.2; §6.8.2; Table 62",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "E303" | "E304" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.2.6",
            spec_path: "docs/specs/02-data-types.md",
        }),
        "W004" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §7.3.3.3.3",
            spec_path: "docs/specs/06-statements.md",
        }),
        "W003" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W005" => Some(DiagnosticExplainer {
            iec_ref: "IEC 61131-3 Ed.3 §6.4.2",
            spec_path: "docs/specs/02-data-types.md",
        }),
        "W008" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W009" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W010" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC); TIME/DATE types per IEC 61131-3 Ed.3 §6.4.2 (Table 10)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W011" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC); Direct variables per IEC 61131-3 Ed.3 §6.5.5 (Table 16)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "W012" => Some(DiagnosticExplainer {
            iec_ref: "Tooling quality lint (non-IEC); shared globals across tasks (IEC 61131-3 Ed.3 §6.5.2.2 Tables 13-16; §6.2/§6.8.2 Table 62)",
            spec_path: "docs/specs/09-semantic-rules.md",
        }),
        "L001" | "L002" | "L003" | "L005" | "L006" | "L007" => Some(DiagnosticExplainer {
            iec_ref: "Tooling config lint (non-IEC)",
            spec_path: "docs/specs/10-runtime.md",
        }),
        _ => None,
    }
}

fn is_oop_access_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("cannot access")
        || lower.contains("access specifier")
        || lower.contains("must be public or internal")
}

fn spec_url(state: &ServerState, spec_path: &str) -> Option<Url> {
    for root in state.workspace_folders() {
        let Some(root_path) = uri_to_path(&root) else {
            continue;
        };
        let candidate = root_path.join(spec_path);
        if candidate.exists() {
            return path_to_uri(&candidate);
        }
    }
    None
}
