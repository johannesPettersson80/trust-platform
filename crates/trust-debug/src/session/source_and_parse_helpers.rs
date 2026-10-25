fn collect_sources(
    path: &str,
    options: &SourceOptions,
) -> Result<Vec<(String, String)>, CompileError> {
    let entry_path = canonicalize_lossy(Path::new(path));
    let root = resolve_root(options, &entry_path)?;
    let include_globs = normalize_globs(&options.include_globs);
    let exclude_globs = normalize_globs(&options.exclude_globs);

    let mut candidates = if include_globs.is_empty() {
        read_folder_sources(&entry_path)?
    } else {
        expand_globs(&root, &include_globs)?
    };

    if !candidates.iter().any(|candidate| candidate == &entry_path) {
        candidates.push(entry_path.clone());
    }

    let (excluded_files, excluded_dirs) = resolve_excludes(&root, &exclude_globs)?;

    let mut unique = HashSet::new();
    let mut sources = Vec::new();
    let ignore_pragmas = resolve_ignore_pragmas(options);

    for candidate in candidates {
        let candidate = canonicalize_lossy(&candidate);
        if !unique.insert(candidate.clone()) {
            continue;
        }
        if !candidate.is_file() {
            continue;
        }
        if !is_structured_text_file(&candidate) {
            continue;
        }
        if candidate != entry_path && is_excluded(&candidate, &excluded_files, &excluded_dirs) {
            continue;
        }
        let content = std::fs::read_to_string(&candidate).map_err(|err| {
            CompileError::new(format!(
                "failed to read source '{}': {err}",
                candidate.display()
            ))
        })?;
        if candidate != entry_path
            && !ignore_pragmas.is_empty()
            && has_ignore_pragma(&content, &ignore_pragmas)
        {
            continue;
        }
        sources.push((candidate.to_string_lossy().to_string(), content));
    }

    if sources.is_empty() {
        let content = std::fs::read_to_string(&entry_path)
            .map_err(|err| CompileError::new(format!("failed to read program: {err}")))?;
        sources.push((entry_path.to_string_lossy().to_string(), content));
    }

    sources.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(sources)
}

/// The sources of the project in the folder the source options name, without a program path: the
/// files the include globs match (every Structured Text file below the folder without them), less
/// the excluded ones and those carrying an ignore pragma, in path order.
fn collect_project_sources(options: &SourceOptions) -> Result<Vec<(String, String)>, CompileError> {
    let root = options
        .root
        .as_ref()
        .map(|root| canonicalize_lossy(Path::new(root)))
        .ok_or_else(|| CompileError::new("no project folder (runtimeRoot or cwd)"))?;
    let mut include_globs = normalize_globs(&options.include_globs);
    if include_globs.is_empty() {
        include_globs.push(String::from("**/*.{st,ST,pou,POU}"));
    }
    let exclude_globs = normalize_globs(&options.exclude_globs);
    let (excluded_files, excluded_dirs) = resolve_excludes(&root, &exclude_globs)?;
    let ignore_pragmas = resolve_ignore_pragmas(options);
    let mut unique = HashSet::new();
    let mut sources = Vec::new();
    for candidate in expand_globs(&root, &include_globs)? {
        let candidate = canonicalize_lossy(&candidate);
        if !unique.insert(candidate.clone())
            || !candidate.is_file()
            || !is_structured_text_file(&candidate)
            || is_excluded(&candidate, &excluded_files, &excluded_dirs)
        {
            continue;
        }
        let content = std::fs::read_to_string(&candidate).map_err(|err| {
            CompileError::new(format!(
                "failed to read source '{}': {err}",
                candidate.display()
            ))
        })?;
        if !ignore_pragmas.is_empty() && has_ignore_pragma(&content, &ignore_pragmas) {
            continue;
        }
        sources.push((candidate.to_string_lossy().to_string(), content));
    }
    sources.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(sources)
}

fn resolve_root(options: &SourceOptions, entry_path: &Path) -> Result<PathBuf, CompileError> {
    if let Some(root) = options.root.as_ref().map(PathBuf::from) {
        return Ok(canonicalize_lossy(&root));
    }
    let parent = entry_path
        .parent()
        .ok_or_else(|| CompileError::new("program path has no parent directory"))?;
    Ok(canonicalize_lossy(parent))
}

fn normalize_globs(globs: &[String]) -> Vec<String> {
    globs
        .iter()
        .map(|glob| glob.trim())
        .filter(|glob| !glob.is_empty())
        .map(|glob| glob.to_string())
        .collect()
}

fn read_folder_sources(entry_path: &Path) -> Result<Vec<PathBuf>, CompileError> {
    let parent = entry_path
        .parent()
        .ok_or_else(|| CompileError::new("program path has no parent directory"))?;
    let read_dir = std::fs::read_dir(parent)
        .map_err(|err| CompileError::new(format!("failed to read project folder: {err}")))?;
    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|err| CompileError::new(format!("read_dir error: {err}")))?;
        let file_path = entry.path();
        if !file_path.is_file() {
            continue;
        }
        entries.push(file_path);
    }
    Ok(entries)
}

fn expand_globs(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, CompileError> {
    let mut matches = Vec::new();
    for pattern in patterns {
        for expanded in expand_braces(pattern) {
            let resolved = resolve_glob_pattern(root, &expanded);
            let entries = glob(&resolved)
                .map_err(|err| CompileError::new(format!("invalid glob '{expanded}': {err}")))?;
            for entry in entries {
                let entry = entry
                    .map_err(|err| CompileError::new(format!("glob error '{expanded}': {err}")))?;
                matches.push(entry);
            }
        }
    }
    Ok(matches)
}

fn expand_braces(pattern: &str) -> Vec<String> {
    let Some((start, end)) = find_brace_range(pattern) else {
        return vec![pattern.to_string()];
    };
    let prefix = &pattern[..start];
    let suffix = &pattern[end + 1..];
    let inner = &pattern[start + 1..end];
    let options = split_brace_options(inner);
    let mut results = Vec::new();
    for option in options {
        let combined = format!("{prefix}{option}{suffix}");
        results.extend(expand_braces(&combined));
    }
    results
}

fn find_brace_range(pattern: &str) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut start = None;
    for (idx, ch) in pattern.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(idx);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return start.map(|s| (s, idx));
                }
            }
            _ => {}
        }
    }
    None
}

fn split_brace_options(inner: &str) -> Vec<String> {
    let mut options = Vec::new();
    let mut depth = 0usize;
    let mut last = 0usize;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
            }
            ',' if depth == 0 => {
                options.push(inner[last..idx].to_string());
                last = idx + 1;
            }
            _ => {}
        }
    }
    options.push(inner[last..].to_string());
    if options.is_empty() {
        options.push(String::new());
    }
    options
}

fn resolve_glob_pattern(root: &Path, pattern: &str) -> String {
    let pattern_path = if Path::new(pattern).is_absolute() {
        PathBuf::from(pattern)
    } else {
        root.join(pattern)
    };
    pattern_path.to_string_lossy().replace('\\', "/")
}

fn resolve_excludes(
    root: &Path,
    patterns: &[String],
) -> Result<(HashSet<PathBuf>, Vec<PathBuf>), CompileError> {
    if patterns.is_empty() {
        return Ok((HashSet::new(), Vec::new()));
    }
    let mut files = HashSet::new();
    let mut dirs = Vec::new();
    for path in expand_globs(root, patterns)? {
        let resolved = canonicalize_lossy(&path);
        if resolved.is_dir() {
            dirs.push(resolved);
        } else {
            files.insert(resolved);
        }
    }
    Ok((files, dirs))
}

fn resolve_ignore_pragmas(options: &SourceOptions) -> Vec<String> {
    match options.ignore_pragmas.as_ref() {
        Some(list) => list
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        None => DEFAULT_IGNORE_PRAGMAS
            .iter()
            .map(|item| item.to_string())
            .collect(),
    }
}

fn is_excluded(path: &Path, excluded_files: &HashSet<PathBuf>, excluded_dirs: &[PathBuf]) -> bool {
    if excluded_files.contains(path) {
        return true;
    }
    excluded_dirs.iter().any(|dir| path.starts_with(dir))
}

fn is_structured_text_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(ext, "st" | "ST" | "pou" | "POU")
}

fn has_ignore_pragma(text: &str, pragmas: &[String]) -> bool {
    if pragmas.is_empty() {
        return false;
    }
    for line in text.lines().take(PRAGMA_SCAN_LINES) {
        for pragma in pragmas {
            if line.contains(pragma) {
                return true;
            }
        }
    }
    false
}

fn canonicalize_lossy(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(canon) = std::fs::canonicalize(path) {
            return strip_windows_device_prefix(canon);
        }
        return strip_windows_device_prefix(path.to_path_buf());
    }

    #[cfg(not(windows))]
    {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }
}

#[cfg(windows)]
fn strip_windows_device_prefix(path: PathBuf) -> PathBuf {
    let raw = match path.to_str() {
        Some(raw) => raw,
        None => return path,
    };

    if let Some(rest) = raw
        .strip_prefix(r"\?\UNC\")
        .or_else(|| raw.strip_prefix(r"\?\UNC\"))
        .or_else(|| raw.strip_prefix(r"\\.\UNC\"))
        .or_else(|| raw.strip_prefix(r"\??\UNC\"))
    {
        let mut unc = String::from(r"\\");
        unc.push_str(rest);
        return PathBuf::from(unc);
    }

    if let Some(rest) = raw
        .strip_prefix(r"\?\")
        .or_else(|| raw.strip_prefix(r"\?\"))
        .or_else(|| raw.strip_prefix(r"\\.\"))
        .or_else(|| raw.strip_prefix(r"\??\"))
    {
        return PathBuf::from(rest);
    }

    path
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
