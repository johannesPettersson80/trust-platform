//! The sources a debug session compiles: the simulator's source selection (specification 13,
//! `stReload`; specification 25 §11.6; specification 38 §3).
//!
//! A session names its project folder (`root`), the include and exclude globs relative to it and
//! the pragmas that keep a file out when one of its first lines (`PRAGMA_SCAN_LINES`) holds one.
//! The project's local package dependencies (`[dependencies]` in its manifest) are added as
//! `trust-runtime build` resolves them. This selection differs from the build's, which takes every
//! file below `src/`: the simulator honours the runtime source options, so that test sources marked
//! `@trustlsp:runtime-ignore` stay out of the running program (specification 38 §3).
//!
//! The debug adapter and the language server both choose a program's sources here, so a check in
//! the editor sees exactly the files the simulator compiles.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
#[cfg(windows)]
use trust_hir::project::strip_windows_device_prefix;

/// The include glob when a session names none: every Structured Text file below the root.
pub const DEFAULT_INCLUDE_GLOB: &str = "**/*.{st,ST,pou,POU}";
/// The pragma that keeps a file out when a session names no pragmas.
pub const DEFAULT_IGNORE_PRAGMAS: &[&str] = &["@trustlsp:runtime-ignore"];
/// How many lines at the top of a file are searched for an ignore pragma.
pub const PRAGMA_SCAN_LINES: usize = 20;

/// The runtime source options of a debug session (specification 13, `stReload`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeSourceOptions {
    /// The project folder the globs are relative to.
    pub root: Option<PathBuf>,
    /// The files to take; none means the default of each selection (see its function).
    pub include_globs: Vec<String>,
    /// Files, or folders, to leave out.
    pub exclude_globs: Vec<String>,
    /// Pragmas that keep a file out; `None` means [`DEFAULT_IGNORE_PRAGMAS`], an empty list
    /// means none.
    pub ignore_pragmas: Option<Vec<String>>,
}

/// The text of a source file. The debug adapter reads the file; the language server gives an open
/// editor's text instead.
pub type ReadSource<'a> = &'a dyn Fn(&Path) -> std::io::Result<String>;

/// Reads a source file from disk.
pub fn read_from_disk(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

/// The sources of a launch or reload of `entry`: the files the include globs match below the root
/// (the files in the entry's folder when the session names no globs), the entry itself, and the
/// sources of the project's local package dependencies; less the excluded files and those that
/// carry an ignore pragma, except the entry, which is always taken. The root is the session's, or
/// else the entry's folder. The list is ordered by path; each path is canonical.
///
/// # Errors
///
/// A glob that does not parse, a dependency that does not resolve or a source that cannot be read.
pub fn launch_sources(
    entry: &Path,
    options: &RuntimeSourceOptions,
    read: ReadSource<'_>,
) -> anyhow::Result<Vec<(String, String)>> {
    let entry_path = canonicalize_lossy(entry);
    let root = match options.root.as_deref() {
        Some(root) => canonicalize_lossy(root),
        None => canonicalize_lossy(
            entry_path
                .parent()
                .ok_or_else(|| anyhow!("program path has no parent directory"))?,
        ),
    };
    let include_globs = normalize_globs(&options.include_globs);
    let exclude_globs = normalize_globs(&options.exclude_globs);

    let mut candidates = if include_globs.is_empty() {
        folder_files(&entry_path)?
    } else {
        expand_globs(&root, &include_globs)?
    };
    if !candidates.iter().any(|candidate| candidate == &entry_path) {
        candidates.push(entry_path.clone());
    }
    // The project's local package dependencies, as `trust-runtime build` resolves them.
    let dependency_roots = crate::bundle_builder::dependency_source_roots(&root)?;
    for dependency_root in dependency_roots {
        candidates.extend(expand_globs(&dependency_root, &["**/*".to_owned()])?);
    }

    let (excluded_files, excluded_dirs) = resolve_excludes(&root, &exclude_globs)?;
    let ignore_pragmas = resolve_ignore_pragmas(options);
    let mut unique = HashSet::new();
    let mut sources = Vec::new();
    for candidate in candidates {
        let candidate = canonicalize_lossy(&candidate);
        if !unique.insert(candidate.clone())
            || !candidate.is_file()
            || !is_structured_text_file(&candidate)
        {
            continue;
        }
        if candidate != entry_path && is_excluded(&candidate, &excluded_files, &excluded_dirs) {
            continue;
        }
        let content = read(&candidate)
            .with_context(|| format!("failed to read source '{}'", candidate.display()))?;
        if candidate != entry_path && has_ignore_pragma(&content, &ignore_pragmas) {
            continue;
        }
        sources.push((candidate.to_string_lossy().to_string(), content));
    }

    if sources.is_empty() {
        let content = read(&entry_path).context("failed to read program")?;
        sources.push((entry_path.to_string_lossy().to_string(), content));
    }

    sources.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(sources)
}

/// The sources of the project in the session's root without an entry: the files the include globs
/// match (every Structured Text file below the root without them), less the excluded ones and those
/// carrying an ignore pragma, ordered by path.
///
/// # Errors
///
/// No root, a glob that does not parse or a source that cannot be read.
pub fn project_sources(
    options: &RuntimeSourceOptions,
    read: ReadSource<'_>,
) -> anyhow::Result<Vec<(String, String)>> {
    let root = options
        .root
        .as_deref()
        .map(canonicalize_lossy)
        .ok_or_else(|| anyhow!("no project folder (runtimeRoot or cwd)"))?;
    let mut include_globs = normalize_globs(&options.include_globs);
    if include_globs.is_empty() {
        include_globs.push(DEFAULT_INCLUDE_GLOB.to_owned());
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
        let content = read(&candidate)
            .with_context(|| format!("failed to read source '{}'", candidate.display()))?;
        if has_ignore_pragma(&content, &ignore_pragmas) {
            continue;
        }
        sources.push((candidate.to_string_lossy().to_string(), content));
    }
    sources.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(sources)
}

/// Whether one of the first [`PRAGMA_SCAN_LINES`] lines of `text` holds one of `pragmas`.
#[must_use]
pub fn has_ignore_pragma(text: &str, pragmas: &[String]) -> bool {
    !pragmas.is_empty()
        && text
            .lines()
            .take(PRAGMA_SCAN_LINES)
            .any(|line| pragmas.iter().any(|pragma| line.contains(pragma.as_str())))
}

/// The pragmas of `options`, trimmed and without empty ones; [`DEFAULT_IGNORE_PRAGMAS`] when the
/// session names none.
#[must_use]
pub fn resolve_ignore_pragmas(options: &RuntimeSourceOptions) -> Vec<String> {
    match options.ignore_pragmas.as_ref() {
        Some(list) => list
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        None => DEFAULT_IGNORE_PRAGMAS
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
    }
}

/// The patterns `pattern` stands for once its braces are expanded: `a{b,c}` is `ab` and `ac`;
/// nested and repeated braces expand too.
#[must_use]
pub fn expand_braces(pattern: &str) -> Vec<String> {
    let Some((start, end)) = find_brace_range(pattern) else {
        return vec![pattern.to_string()];
    };
    let prefix = &pattern[..start];
    let suffix = &pattern[end + 1..];
    let inner = &pattern[start + 1..end];
    let mut results = Vec::new();
    for option in split_brace_options(inner) {
        results.extend(expand_braces(&format!("{prefix}{option}{suffix}")));
    }
    results
}

/// The canonical form of `path`, or `path` itself when it cannot be resolved. On Windows the
/// device prefix (`\\?\`) is dropped, so a path compares with one a client gives.
#[must_use]
pub fn canonicalize_lossy(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(canon) = std::fs::canonicalize(path) {
            return strip_windows_device_prefix(canon);
        }
        strip_windows_device_prefix(path.to_path_buf())
    }

    #[cfg(not(windows))]
    {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }
}

fn normalize_globs(globs: &[String]) -> Vec<String> {
    globs
        .iter()
        .map(|glob| glob.trim())
        .filter(|glob| !glob.is_empty())
        .map(str::to_string)
        .collect()
}

/// The files in the folder of `entry_path`, without its subfolders.
fn folder_files(entry_path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let parent = entry_path
        .parent()
        .ok_or_else(|| anyhow!("program path has no parent directory"))?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(parent).context("failed to read project folder")? {
        let path = entry.context("read_dir error")?.path();
        if path.is_file() {
            entries.push(path);
        }
    }
    Ok(entries)
}

fn expand_globs(root: &Path, patterns: &[String]) -> anyhow::Result<Vec<PathBuf>> {
    let mut matches = Vec::new();
    for pattern in patterns {
        for expanded in expand_braces(pattern) {
            let resolved = resolve_glob_pattern(root, &expanded);
            let entries =
                glob::glob(&resolved).with_context(|| format!("invalid glob '{expanded}'"))?;
            for entry in entries {
                matches.push(entry.with_context(|| format!("glob error '{expanded}'"))?);
            }
        }
    }
    Ok(matches)
}

fn find_brace_range(pattern: &str) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut start = None;
    for (index, ch) in pattern.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return start.map(|start| (start, index));
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
    for (index, ch) in inner.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                options.push(inner[last..index].to_string());
                last = index + 1;
            }
            _ => {}
        }
    }
    options.push(inner[last..].to_string());
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
) -> anyhow::Result<(HashSet<PathBuf>, Vec<PathBuf>)> {
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

fn is_excluded(path: &Path, excluded_files: &HashSet<PathBuf>, excluded_dirs: &[PathBuf]) -> bool {
    excluded_files.contains(path) || excluded_dirs.iter().any(|dir| path.starts_with(dir))
}

fn is_structured_text_file(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    matches!(extension, "st" | "ST" | "pou" | "POU")
}

#[cfg(test)]
#[path = "runtime_sources_tests.rs"]
mod tests;
