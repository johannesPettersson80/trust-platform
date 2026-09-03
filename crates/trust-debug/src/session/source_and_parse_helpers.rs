#[derive(Clone, Copy)]
struct BreakpointContext<'a> {
    sources: &'a HashMap<SourceKey, SourceFile>,
    metadata: &'a RuntimeMetadata,
    control: &'a DebugControl,
}

impl<'a> BreakpointContext<'a> {
    fn new(
        sources: &'a HashMap<SourceKey, SourceFile>,
        metadata: &'a RuntimeMetadata,
        control: &'a DebugControl,
    ) -> Self {
        Self {
            sources,
            metadata,
            control,
        }
    }
}

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

fn apply_project_io_config(
    runtime: &mut Runtime,
    program_path: &str,
    options: &SourceOptions,
) -> Result<(), CompileError> {
    let Some(project_root) = resolve_project_root_for_config(program_path, options)? else {
        return Ok(());
    };
    let io_path = project_root.join("io.toml");
    if !io_path.is_file() {
        return Ok(());
    }

    let io = IoConfig::load(&io_path)
        .map_err(|err| CompileError::new(format!("failed to load io.toml: {err}")))?;
    runtime.clear_io_drivers();
    runtime.set_io_safe_state(io.safe_state);
    let drivers = io.drivers;
    let registry = IoDriverRegistry::default_registry();
    let mut resolved_drivers = Vec::with_capacity(drivers.len());
    for driver in &drivers {
        let mut resolved_driver = driver.clone();
        if !driver.enabled {
            resolved_drivers.push(resolved_driver);
            continue;
        }
        if driver.name.eq_ignore_ascii_case("mqtt")
            || driver.name.eq_ignore_ascii_case("mqtt-tcp")
        {
            resolved_driver.params = trust_runtime::io::resolve_mqtt_tag_mappings(
                runtime,
                &resolved_driver.params,
            )
            .map_err(|err| CompileError::new(format!("failed to load io.toml: {err}")))?;
        }
        if let Some(spec) = registry
            .build(resolved_driver.name.as_str(), &resolved_driver.params)
            .map_err(|err| CompileError::new(format!("failed to load io.toml: {err}")))?
        {
            runtime.add_io_driver(spec.name, spec.driver);
        }
        resolved_drivers.push(resolved_driver);
    }
    trust_runtime::io::annotate_io_binding_sources(runtime.io_mut(), &resolved_drivers);
    Ok(())
}

#[cfg(feature = "ads-wire")]
fn apply_project_ads_config(
    runtime: &mut Runtime,
    program_path: &str,
    options: &SourceOptions,
) -> Result<(), CompileError> {
    let Some(project_root) = resolve_project_root_for_config(program_path, options)? else {
        return Ok(());
    };
    let runtime_path = project_root.join("runtime.toml");
    if !runtime_path.is_file() {
        return Ok(());
    }

    let runtime_config = trust_runtime::config::RuntimeConfig::load(&runtime_path)
        .map_err(|err| CompileError::new(format!("failed to load runtime.toml: {err}")))?;
    if !runtime_config.ads.enabled {
        return Ok(());
    }

    let ads_path = if runtime_config.ads.config_path.is_relative() {
        project_root.join(&runtime_config.ads.config_path)
    } else {
        runtime_config.ads.config_path.clone()
    };
    let text = std::fs::read_to_string(&ads_path).map_err(|err| {
        CompileError::new(format!("failed to load {}: {err}", ads_path.display()))
    })?;
    let config = trust_runtime::ads::parse_ads_toml(&text)
        .map_err(|err| CompileError::new(format!("failed to parse ads.toml: {err}")))?;
    let config_hash = trust_runtime::ads::diagnostics::sha256_evidence_hash(text.as_bytes());
    runtime
        .shutdown_ads()
        .map_err(|err| CompileError::new(format!("failed to reset ADS runtime: {err}")))?;
    runtime.set_ads_deployed_config_hash(Some(config_hash));
    let worker_tick_interval = std::time::Duration::from_millis(
        u64::try_from(runtime_config.ads.worker_tick_interval.as_millis()).unwrap_or(20),
    );
    for connection in &config.connections {
        let transport = trust_runtime::ads::HostAdsClient::new(connection.route.clone());
        runtime
            .start_ads_connection(connection, transport, worker_tick_interval)
            .map_err(|err| CompileError::new(format!("failed to start ADS runtime: {err}")))?;
    }
    Ok(())
}

#[cfg(not(feature = "ads-wire"))]
fn apply_project_ads_config(
    _runtime: &mut Runtime,
    program_path: &str,
    options: &SourceOptions,
) -> Result<(), CompileError> {
    let Some(project_root) = resolve_project_root_for_config(program_path, options)? else {
        return Ok(());
    };
    let runtime_path = project_root.join("runtime.toml");
    if !runtime_path.is_file() {
        return Ok(());
    }
    let runtime_config = trust_runtime::config::RuntimeConfig::load(&runtime_path)
        .map_err(|err| CompileError::new(format!("failed to load runtime.toml: {err}")))?;
    if runtime_config.ads.enabled {
        return Err(CompileError::new(
            "runtime.ads.enabled=true requires trust-debug built with feature 'ads-wire'",
        ));
    }
    Ok(())
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
    if let Some(project_root) = parent.parent() {
        if project_root.join("io.toml").is_file() || project_root.join("runtime.toml").is_file() {
            return Ok(Some(project_root.to_path_buf()));
        }
    }
    Ok(None)
}

fn source_key_for_path(path: &str) -> SourceKey {
    let normalized = canonicalize_lossy(Path::new(path));
    SourceKey::from_path(normalized)
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

fn requested_breakpoints(args: &SetBreakpointsArguments) -> Vec<SourceBreakpoint> {
    if let Some(breakpoints) = &args.breakpoints {
        return breakpoints.clone();
    }
    let Some(lines) = &args.lines else {
        return Vec::new();
    };
    lines
        .iter()
        .map(|line| SourceBreakpoint {
            line: *line,
            column: None,
            condition: None,
            hit_condition: None,
            log_message: None,
        })
        .collect()
}

fn parse_hit_condition(raw: &str) -> Option<HitCondition> {
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

fn parse_log_message(
    template: &str,
    registry: &mut trust_hir::types::TypeRegistry,
    profile: trust_runtime::value::DateTimeProfile,
    using: &[smol_str::SmolStr],
) -> Result<Vec<LogFragment>, String> {
    let mut fragments = Vec::new();
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
                    fragments.push(LogFragment::Text(std::mem::take(&mut literal)));
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
                let compiled = parse_debug_expression(expr, registry, profile, using)
                    .map_err(|err| err.to_string())?;
                fragments.push(LogFragment::Expr(compiled));
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
        fragments.push(LogFragment::Text(literal));
    }

    Ok(fragments)
}

fn to_zero_based(line: u32, column: Option<u32>) -> Option<(u32, u32)> {
    if line == 0 {
        return None;
    }
    let column = column.unwrap_or(1);
    if column == 0 {
        return None;
    }
    Some((line.saturating_sub(1), column.saturating_sub(1)))
}

fn first_non_whitespace_column(source: &str, line: u32) -> Option<u32> {
    let line_idx = usize::try_from(line).ok()?;
    let line_str = source.lines().nth(line_idx)?;
    let mut col = 0u32;
    for ch in line_str.chars() {
        if !ch.is_whitespace() {
            return Some(col);
        }
        col = col.saturating_add(ch.len_utf8() as u32);
    }
    Some(0)
}
