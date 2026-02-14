//! Bundle build helpers (compile sources to program.stbc).

use anyhow::Context;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{CompileSession, SourceFile};

const DEPENDENCY_MANIFEST_FILES: &[&str] = &["trust-lsp.toml", ".trust-lsp.toml", "trustlsp.toml"];

/// Build output summary for a bundle.
#[derive(Debug, Clone)]
pub struct BundleBuildReport {
    /// Written bytecode path (program.stbc).
    pub program_path: PathBuf,
    /// Source files included in the build.
    pub sources: Vec<PathBuf>,
    /// Resolved dependency roots included in this build.
    pub dependency_roots: Vec<PathBuf>,
    /// Resolved dependency names in deterministic order.
    pub resolved_dependencies: Vec<String>,
}

/// Compile bundle sources into `program.stbc`.
pub fn build_program_stbc(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<BundleBuildReport> {
    let sources_root = resolve_sources_root(bundle_root, sources_root)?;

    let dependencies = resolve_local_dependencies(bundle_root)?;
    let mut source_roots = vec![sources_root.clone()];
    for dependency in &dependencies {
        source_roots.push(preferred_dependency_sources_root(&dependency.path));
    }

    let (sources, source_paths) = collect_sources(&source_roots)?;
    if sources.is_empty() {
        anyhow::bail!(
            "no source files found in {} (expected .st/.pou files)",
            sources_root.display()
        );
    }

    let session = CompileSession::from_sources(sources);
    let bytes = session.build_bytecode_bytes()?;
    fs::create_dir_all(bundle_root)?;
    let program_path = bundle_root.join("program.stbc");
    fs::write(&program_path, bytes)?;

    Ok(BundleBuildReport {
        program_path,
        sources: source_paths,
        dependency_roots: dependencies
            .iter()
            .map(|dependency| dependency.path.clone())
            .collect(),
        resolved_dependencies: dependencies
            .iter()
            .map(|dependency| dependency.name.clone())
            .collect(),
    })
}

/// Resolve the effective project source root for bundle operations.
///
/// Behavior:
/// - if `sources_root` is provided and relative, it is resolved relative to `bundle_root`
/// - default search uses `src/`
pub fn resolve_sources_root(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(override_root) = sources_root {
        let resolved = if override_root.is_absolute() {
            override_root.to_path_buf()
        } else {
            bundle_root.join(override_root)
        };
        let resolved = canonicalize_or_self(&resolved);
        if !resolved.is_dir() {
            anyhow::bail!("sources directory not found: {}", resolved.display());
        }
        return Ok(resolved);
    }

    let src_root = bundle_root.join("src");
    if src_root.is_dir() {
        return Ok(canonicalize_or_self(&src_root));
    }

    anyhow::bail!(
        "invalid project folder '{}': missing src/ directory",
        bundle_root.display()
    );
}

fn resolve_local_dependencies(bundle_root: &Path) -> anyhow::Result<Vec<ResolvedDependency>> {
    let manifest = load_dependency_manifest(bundle_root).with_context(|| {
        format!(
            "failed to load dependency manifest at {}",
            bundle_root.display()
        )
    })?;
    let declared = parse_dependency_specs(bundle_root, &manifest.dependencies);

    let mut resolved = BTreeMap::new();
    let mut states = HashMap::new();
    let mut stack = Vec::new();
    for dependency in &declared {
        resolve_dependency_recursive(dependency, &mut states, &mut stack, &mut resolved)?;
    }
    Ok(resolved.into_values().collect())
}

fn resolve_dependency_recursive(
    dependency: &DependencySpec,
    states: &mut HashMap<String, DependencyVisitState>,
    stack: &mut Vec<String>,
    resolved: &mut BTreeMap<String, ResolvedDependency>,
) -> anyhow::Result<()> {
    let path = canonicalize_or_self(&dependency.path);
    if !path.is_dir() {
        anyhow::bail!(
            "dependency '{}' path does not exist: {}",
            dependency.name,
            path.display()
        );
    }
    let dependency_src = path.join("src");
    if !dependency_src.is_dir() {
        anyhow::bail!(
            "dependency '{}' missing src/ directory: {}",
            dependency.name,
            dependency_src.display()
        );
    }

    if let Some(existing) = resolved.get(&dependency.name) {
        ensure_dependency_version(
            dependency.name.as_str(),
            dependency.version.as_deref(),
            existing.version.as_deref(),
        )?;
        return Ok(());
    }

    match states.get(dependency.name.as_str()).copied() {
        Some(DependencyVisitState::Visiting) => {
            let mut cycle = stack.clone();
            cycle.push(dependency.name.clone());
            anyhow::bail!("cyclic dependency detected: {}", cycle.join(" -> "));
        }
        Some(DependencyVisitState::Done) => return Ok(()),
        None => {}
    }

    states.insert(dependency.name.clone(), DependencyVisitState::Visiting);
    stack.push(dependency.name.clone());

    let manifest = load_dependency_manifest(&path).with_context(|| {
        format!(
            "failed to load dependency manifest for '{}' ({})",
            dependency.name,
            path.display()
        )
    })?;
    ensure_dependency_version(
        dependency.name.as_str(),
        dependency.version.as_deref(),
        manifest.package.version.as_deref(),
    )?;

    let nested = parse_dependency_specs(&path, &manifest.dependencies);
    for nested_dependency in &nested {
        resolve_dependency_recursive(nested_dependency, states, stack, resolved)?;
    }

    resolved.insert(
        dependency.name.clone(),
        ResolvedDependency {
            name: dependency.name.clone(),
            path,
            version: manifest.package.version,
        },
    );
    let _ = stack.pop();
    states.insert(dependency.name.clone(), DependencyVisitState::Done);
    Ok(())
}

fn ensure_dependency_version(
    name: &str,
    required: Option<&str>,
    actual: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(required) = required {
        if actual != Some(required) {
            let resolved = actual.unwrap_or("unspecified");
            anyhow::bail!(
                "dependency '{}' requested version {}, but resolved package version is {}",
                name,
                required,
                resolved
            );
        }
    }
    Ok(())
}

fn parse_dependency_specs(
    root: &Path,
    entries: &BTreeMap<String, ManifestDependencyEntry>,
) -> Vec<DependencySpec> {
    entries
        .iter()
        .map(|(name, entry)| DependencySpec {
            name: name.clone(),
            path: resolve_path(root, entry.path()),
            version: entry.version(),
        })
        .collect()
}

fn preferred_dependency_sources_root(path: &Path) -> PathBuf {
    path.join("src")
}

fn collect_sources(source_roots: &[PathBuf]) -> anyhow::Result<(Vec<SourceFile>, Vec<PathBuf>)> {
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    let mut seen = BTreeSet::new();
    let mut source_map = BTreeMap::new();

    for root in source_roots {
        if !root.is_dir() {
            continue;
        }
        for pattern in patterns {
            for entry in glob::glob(&format!("{}/{}", root.display(), pattern))? {
                let path = entry?;
                if !path.is_file() {
                    continue;
                }
                let resolved = canonicalize_or_self(&path);
                let path_text = resolved.to_string_lossy().to_string();
                if !seen.insert(path_text.clone()) {
                    continue;
                }
                let text = fs::read_to_string(&resolved)?;
                source_map.insert(path_text, text);
            }
        }
    }

    let mut sources = Vec::with_capacity(source_map.len());
    let mut paths = Vec::with_capacity(source_map.len());
    for (path, text) in source_map {
        paths.push(PathBuf::from(&path));
        sources.push(SourceFile::with_path(path, text));
    }
    Ok((sources, paths))
}

fn load_dependency_manifest(root: &Path) -> anyhow::Result<DependencyManifestFile> {
    let Some(path) = find_dependency_manifest(root) else {
        return Ok(DependencyManifestFile::default());
    };
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read dependency manifest {}", path.display()))?;
    let parsed = toml::from_str(&contents)
        .with_context(|| format!("failed to parse dependency manifest {}", path.display()))?;
    Ok(parsed)
}

fn find_dependency_manifest(root: &Path) -> Option<PathBuf> {
    DEPENDENCY_MANIFEST_FILES
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn resolve_path(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[derive(Debug, Clone)]
struct DependencySpec {
    name: String,
    path: PathBuf,
    version: Option<String>,
}

#[derive(Debug, Clone)]
struct ResolvedDependency {
    name: String,
    path: PathBuf,
    version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyVisitState {
    Visiting,
    Done,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyManifestFile {
    #[serde(default)]
    package: PackageSection,
    #[serde(default)]
    dependencies: BTreeMap<String, ManifestDependencyEntry>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageSection {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ManifestDependencyEntry {
    Path(String),
    Detailed(ManifestDependencySection),
}

impl ManifestDependencyEntry {
    fn path(&self) -> &str {
        match self {
            ManifestDependencyEntry::Path(path) => path,
            ManifestDependencyEntry::Detailed(section) => section.path.as_str(),
        }
    }

    fn version(&self) -> Option<String> {
        match self {
            ManifestDependencyEntry::Path(_) => None,
            ManifestDependencyEntry::Detailed(section) => section.version.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ManifestDependencySection {
    path: String,
    version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, content).expect("write file");
    }

    fn write_root_source(root: &Path) {
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR
    y : INT;
END_VAR
y := DepDouble(2);
END_PROGRAM
"#,
        );
    }

    fn write_dependency_source(root: &Path, name: &str) {
        write_file(
            &root.join("src/lib.st"),
            &format!(
                r#"
FUNCTION {name} : INT
VAR_INPUT
    x : INT;
END_VAR
{name} := x * 2;
END_FUNCTION
"#
            ),
        );
    }

    #[test]
    fn build_includes_transitive_dependency_sources() {
        let root = temp_dir("trust-runtime-build-deps");
        let dep_a = root.join("deps/lib-a");
        let dep_b = root.join("deps/lib-b");
        write_root_source(&root);
        write_dependency_source(&dep_a, "DepDouble");
        write_dependency_source(&dep_b, "DepTriple");
        write_file(
            &root.join("trust-lsp.toml"),
            r#"
[dependencies]
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
        );
        write_file(
            &dep_a.join("trust-lsp.toml"),
            r#"
[package]
version = "1.0.0"

[dependencies]
LibB = { path = "../lib-b", version = "2.0.0" }
"#,
        );
        write_file(
            &dep_b.join("trust-lsp.toml"),
            r#"
[package]
version = "2.0.0"
"#,
        );

        let report = build_program_stbc(&root, None).expect("build should pass");
        assert!(report.program_path.exists());
        assert!(report.sources.iter().any(|path| path.ends_with("main.st")));
        assert!(report.sources.iter().any(|path| path.ends_with("lib.st")));
        assert_eq!(
            report.resolved_dependencies,
            vec!["LibA".to_string(), "LibB".to_string()]
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn build_fails_for_missing_dependency_path() {
        let root = temp_dir("trust-runtime-build-missing");
        write_root_source(&root);
        write_file(
            &root.join("trust-lsp.toml"),
            r#"
[dependencies]
Missing = "deps/missing"
"#,
        );

        let err = build_program_stbc(&root, None).expect_err("build should fail");
        let message = err.to_string();
        assert!(message.contains("dependency 'Missing' path does not exist"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn build_fails_for_cyclic_dependencies() {
        let root = temp_dir("trust-runtime-build-cycle");
        let dep_a = root.join("deps/lib-a");
        let dep_b = root.join("deps/lib-b");
        write_root_source(&root);
        write_dependency_source(&dep_a, "DepDouble");
        write_dependency_source(&dep_b, "DepTriple");
        write_file(
            &root.join("trust-lsp.toml"),
            r#"
[dependencies]
LibA = { path = "deps/lib-a" }
"#,
        );
        write_file(
            &dep_a.join("trust-lsp.toml"),
            r#"
[dependencies]
LibB = { path = "../lib-b" }
"#,
        );
        write_file(
            &dep_b.join("trust-lsp.toml"),
            r#"
[dependencies]
LibA = { path = "../lib-a" }
"#,
        );

        let err = build_program_stbc(&root, None).expect_err("build should fail");
        let message = err.to_string();
        assert!(message.contains("cyclic dependency detected"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn build_fails_for_version_mismatch() {
        let root = temp_dir("trust-runtime-build-version");
        let dep_a = root.join("deps/lib-a");
        write_root_source(&root);
        write_dependency_source(&dep_a, "DepDouble");
        write_file(
            &root.join("trust-lsp.toml"),
            r#"
[dependencies]
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
        );
        write_file(
            &dep_a.join("trust-lsp.toml"),
            r#"
[package]
version = "2.0.0"
"#,
        );

        let err = build_program_stbc(&root, None).expect_err("build should fail");
        let message = err.to_string();
        assert!(message.contains("requested version 1.0.0"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn dependency_resolution_order_is_deterministic() {
        let root = temp_dir("trust-runtime-build-deterministic");
        let dep_a = root.join("deps/lib-a");
        let dep_b = root.join("deps/lib-b");
        write_file(
            &root.join("src/main.st"),
            r#"
PROGRAM Main
VAR
    a : INT;
    b : INT;
END_VAR
a := ADouble(1);
b := BDouble(2);
END_PROGRAM
"#,
        );
        write_dependency_source(&dep_a, "ADouble");
        write_dependency_source(&dep_b, "BDouble");
        write_file(
            &root.join("trust-lsp.toml"),
            r#"
[dependencies]
LibB = { path = "deps/lib-b", version = "1.0.0" }
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
        );
        write_file(
            &dep_a.join("trust-lsp.toml"),
            r#"
[package]
version = "1.0.0"
"#,
        );
        write_file(
            &dep_b.join("trust-lsp.toml"),
            r#"
[package]
version = "1.0.0"
"#,
        );

        let first = build_program_stbc(&root, None).expect("first build");
        let first_bytes = fs::read(&first.program_path).expect("read first program");
        let second = build_program_stbc(&root, None).expect("second build");
        let second_bytes = fs::read(&second.program_path).expect("read second program");

        assert_eq!(first.resolved_dependencies, second.resolved_dependencies);
        assert_eq!(first.sources, second.sources);
        assert_eq!(first_bytes, second_bytes);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn resolve_sources_root_prefers_src_directory() {
        let root = temp_dir("trust-runtime-resolve-src");
        write_file(&root.join("src/main.st"), "PROGRAM Main END_PROGRAM");

        let resolved = resolve_sources_root(&root, None).expect("resolve sources root");
        assert!(resolved.ends_with("src"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn resolve_sources_root_rejects_legacy_sources_directory() {
        let root = temp_dir("trust-runtime-resolve-sources");
        write_file(&root.join("sources/main.st"), "PROGRAM Legacy END_PROGRAM");
        let err = resolve_sources_root(&root, None).expect_err("legacy sources should fail");
        let message = err.to_string();
        assert!(message.contains("missing src/ directory"));

        fs::remove_dir_all(root).ok();
    }
}
