use super::{ConfigFile, IndexingConfig, StdlibSelection};

pub(crate) struct ConfigurationIssue {
    pub(crate) code: &'static str,
    pub(crate) subject: String,
    pub(crate) message: String,
}

pub(crate) fn configuration_issues(contents: &str) -> Vec<ConfigurationIssue> {
    let parsed: ConfigFile = match toml::from_str(contents) {
        Ok(parsed) => parsed,
        Err(error) => {
            return vec![issue(
                "C001",
                "",
                format!("failed to parse configuration: {error}"),
            )];
        }
    };

    let mut issues = Vec::new();
    if let StdlibSelection::Profile(profile) = &parsed.project.stdlib {
        let normalized = profile.trim().to_ascii_lowercase();
        if !matches!(normalized.as_str(), "full" | "iec" | "none") {
            issues.push(issue(
                "C002",
                profile,
                format!("unknown standard-library profile '{profile}'; using 'full'"),
            ));
        }
    }

    for (subject, qualified_name, value) in [
        ("max_files", "indexing.max_files", parsed.indexing.max_files),
        (
            "memory_budget_mb",
            "indexing.memory_budget_mb",
            parsed.indexing.memory_budget_mb,
        ),
    ] {
        if value == Some(0) {
            issues.push(issue(
                "C003",
                subject,
                format!(
                    "{qualified_name} must be positive when configured; using the unbounded default"
                ),
            ));
        }
    }
    if parsed.indexing.max_ms == Some(0) {
        issues.push(issue(
            "C003",
            "max_ms",
            "indexing.max_ms must be positive when configured; using the unbounded default",
        ));
    }
    if parsed.indexing.throttle_active_window_ms == Some(0) {
        issues.push(issue(
            "C003",
            "throttle_active_window_ms",
            "indexing.throttle_active_window_ms must be positive when configured; using the default",
        ));
    }
    if parsed
        .indexing
        .evict_to_percent
        .is_some_and(|value| !(1..=100).contains(&value))
    {
        issues.push(issue(
            "C003",
            "evict_to_percent",
            "indexing.evict_to_percent must be in 1..=100; clamping to the nearest safe bound",
        ));
    }
    let defaults = IndexingConfig::default();
    let throttle_idle_ms = parsed
        .indexing
        .throttle_idle_ms
        .unwrap_or(defaults.throttle_idle_ms);
    let throttle_active_ms = parsed
        .indexing
        .throttle_active_ms
        .unwrap_or(defaults.throttle_active_ms);
    let throttle_max_ms = parsed
        .indexing
        .throttle_max_ms
        .unwrap_or(defaults.throttle_max_ms);
    if throttle_idle_ms > throttle_active_ms || throttle_active_ms > throttle_max_ms {
        issues.push(issue(
            "C003",
            "throttle_idle_ms",
            "indexing throttle delays are incoherent; using the complete default throttle tuple",
        ));
    }
    if parsed.telemetry.flush_every == Some(0) {
        issues.push(issue(
            "C003",
            "flush_every",
            "telemetry.flush_every must be positive when configured; using the default",
        ));
    }

    if let Some(visibility) = &parsed.workspace.visibility {
        if !matches!(
            visibility.trim().to_ascii_lowercase().as_str(),
            "public" | "private" | "hidden"
        ) {
            issues.push(issue(
                "C004",
                visibility,
                format!("unknown workspace.visibility '{visibility}'; using 'public'"),
            ));
        }
    }

    for (code, severity) in &parsed.diagnostics.severity_overrides {
        if code.trim().is_empty() {
            issues.push(issue(
                "C005",
                code,
                "diagnostic override code must not be blank; entry ignored",
            ));
        }
        if !matches!(
            severity.trim().to_ascii_lowercase().as_str(),
            "error" | "err" | "warning" | "warn" | "info" | "information" | "hint"
        ) {
            issues.push(issue(
                "C005",
                severity,
                format!("unknown diagnostic severity '{severity}'; entry ignored"),
            ));
        }
    }
    issues
}

fn issue(
    code: &'static str,
    subject: impl Into<String>,
    message: impl Into<String>,
) -> ConfigurationIssue {
    ConfigurationIssue {
        code,
        subject: subject.into(),
        message: message.into(),
    }
}
