/// The sources of a launch or reload of `path`: the simulator's source selection, which the
/// language server shares (`trust_runtime::runtime_sources`, specification 38 §3).
fn collect_sources(
    path: &str,
    options: &SourceOptions,
) -> Result<Vec<(String, String)>, CompileError> {
    trust_runtime::runtime_sources::launch_sources(
        Path::new(path),
        &runtime_source_options(options),
        &trust_runtime::runtime_sources::read_from_disk,
    )
    .map_err(|error| CompileError::new(format!("{error:#}")))
}

/// The sources of the project in the folder the source options name, without a program path.
fn collect_project_sources(options: &SourceOptions) -> Result<Vec<(String, String)>, CompileError> {
    trust_runtime::runtime_sources::project_sources(
        &runtime_source_options(options),
        &trust_runtime::runtime_sources::read_from_disk,
    )
    .map_err(|error| CompileError::new(format!("{error:#}")))
}

fn runtime_source_options(
    options: &SourceOptions,
) -> trust_runtime::runtime_sources::RuntimeSourceOptions {
    trust_runtime::runtime_sources::RuntimeSourceOptions {
        root: options.root.as_ref().map(PathBuf::from),
        include_globs: options.include_globs.clone(),
        exclude_globs: options.exclude_globs.clone(),
        ignore_pragmas: options.ignore_pragmas.clone(),
    }
}

/// The conversion rules of the debugged project's `vendor_profile`
/// (docs/specs/07-standard-functions.md 2.10), so a debug session computes what
/// `trust-runtime run` computes. The project is found like its io.toml and runtime.toml, or
/// else as the program's folder or its parent when that holds the project manifest.
fn project_conversion_profile_for(
    program_path: &str,
    options: &SourceOptions,
) -> Result<ConversionProfile, CompileError> {
    let root = match resolve_project_root_for_config(program_path, options)? {
        Some(root) => Some(root),
        None => {
            // the program's folder and its parent
            let entry_path = canonicalize_lossy(Path::new(program_path));
            entry_path
                .ancestors()
                .skip(1)
                .take(2)
                .find(|candidate| find_project_manifest(candidate).is_some())
                .map(Path::to_path_buf)
        }
    };
    match root {
        Some(root) => trust_runtime::native::conversion_profile(&root)
            .map_err(|error| CompileError::new(error.to_string())),
        None => Ok(ConversionProfile::Iec),
    }
}

fn resolve_project_root_for_config(
    program_path: &str,
    options: &SourceOptions,
) -> Result<Option<PathBuf>, CompileError> {
    if let Some(root) = options.root.as_ref().map(PathBuf::from) {
        return Ok(Some(canonicalize_lossy(&root)));
    }

    let entry_path = canonicalize_lossy(Path::new(program_path));
    let parent = entry_path
        .parent()
        .ok_or_else(|| CompileError::new("program path has no parent directory"))?;
    if parent.join("io.toml").is_file() || parent.join("runtime.toml").is_file() {
        return Ok(Some(parent.to_path_buf()));
    }
    if let Some(project_root) = parent.parent()
        && (project_root.join("io.toml").is_file() || project_root.join("runtime.toml").is_file())
    {
        return Ok(Some(project_root.to_path_buf()));
    }
    Ok(None)
}

fn canonicalize_lossy(path: &Path) -> PathBuf {
    trust_runtime::runtime_sources::canonicalize_lossy(path)
}

pub(crate) fn parse_hit_condition(raw: &str) -> Option<HitCondition> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (op, rest) = if let Some(rest) = trimmed.strip_prefix(">=") {
        ("ge", rest)
    } else if let Some(rest) = trimmed.strip_prefix("==") {
        ("eq", rest)
    } else if let Some(rest) = trimmed.strip_prefix('>') {
        ("gt", rest)
    } else {
        ("eq", trimmed)
    };
    let value: u64 = rest.trim().parse().ok()?;
    if value == 0 {
        return None;
    }
    match op {
        "ge" => Some(HitCondition::AtLeast(value)),
        "gt" => Some(HitCondition::GreaterThan(value)),
        _ => Some(HitCondition::Equal(value)),
    }
}

/// One part of a logpoint's message template: literal text, or the text of an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogText {
    Text(String),
    Expression(String),
}

/// The parts of the logpoint message `template` (specification 13): `{expression}` encloses an
/// expression, and `{{` and `}}` stand for a brace.
pub(crate) fn split_log_message(template: &str) -> Result<Vec<LogText>, String> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    literal.push('{');
                    continue;
                }
                if !literal.is_empty() {
                    parts.push(LogText::Text(std::mem::take(&mut literal)));
                }
                let mut expr = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    expr.push(next);
                }
                if !closed {
                    return Err("unterminated '{' in log message".to_string());
                }
                let expr = expr.trim();
                if expr.is_empty() {
                    return Err("empty log expression".to_string());
                }
                parts.push(LogText::Expression(expr.to_string()));
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    literal.push('}');
                } else {
                    return Err("unmatched '}' in log message".to_string());
                }
            }
            _ => literal.push(ch),
        }
    }

    if !literal.is_empty() {
        parts.push(LogText::Text(literal));
    }

    Ok(parts)
}
