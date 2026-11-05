use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tracing::warn;

/// How a project's configuration is loaded. The editor and `trust-lsp check` load it as written
/// (the default); an analysis for the agent API loads it contained, so that it writes nothing
/// and fetches nothing, whatever the file says (specification 20 §7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LoadOptions {
    /// Never clone or fetch a dependency, as `[build] dependencies_offline = true` does: a git
    /// dependency is used only when its checkout already exists.
    pub(crate) offline_dependencies: bool,
    /// Never read or write the index cache, as `[indexing] cache = false` does.
    pub(crate) no_index_cache: bool,
}

impl super::ProjectConfig {
    /// Load configuration for a workspace root as written. The language server loads through its
    /// state's options (`load_with`).
    #[cfg(test)]
    pub fn load(root: &Path) -> Self {
        Self::load_with(root, LoadOptions::default())
    }

    /// Load configuration for a workspace root as `options` say.
    pub(crate) fn load_with(root: &Path, options: LoadOptions) -> Self {
        let config_path = super::find_config_file(root);
        let Some(path) = config_path.clone() else {
            return super::ProjectConfig::base(root, None).limited_by(options);
        };
        let Ok(contents) = std::fs::read_to_string(&path) else {
            warn!("Failed to read trust-lsp config at {}", path.display());
            return super::ProjectConfig::base(root, config_path).limited_by(options);
        };
        super::ProjectConfig::from_contents_with(root, config_path, &contents, options)
    }

    /// Why `contents` cannot be read as a project's `trust-lsp.toml`: the one-based line the
    /// parser stopped at and its message. `None` when [`Self::from_contents_with`] reads it
    /// (specification 25 §6.2).
    pub(crate) fn read_problem(contents: &str) -> Option<(usize, String)> {
        let error = toml::from_str::<super::ConfigFile>(contents).err()?;
        let line = error.span().map_or(1, |span| {
            let end = span.start.min(contents.len());
            contents.as_bytes()[..end]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
                + 1
        });
        Some((line, error.message().trim().to_owned()))
    }

    /// Read `contents` as a project's `trust-lsp.toml` as written. The language server reads it
    /// through its state's options (`from_contents_with`).
    #[cfg(test)]
    pub fn from_contents(root: &Path, config_path: Option<PathBuf>, contents: &str) -> Self {
        Self::from_contents_with(root, config_path, contents, LoadOptions::default())
    }

    /// Read `contents` as a project's `trust-lsp.toml`, as `options` say.
    pub(crate) fn from_contents_with(
        root: &Path,
        config_path: Option<PathBuf>,
        contents: &str,
        options: LoadOptions,
    ) -> Self {
        let mut config = super::ProjectConfig::base(root, config_path).limited_by(options);
        let parsed: super::ConfigFile = match toml::from_str(contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                if let Some(path) = &config.config_path {
                    warn!(
                        "Failed to parse trust-lsp config at {}: {err}",
                        path.display()
                    );
                } else {
                    warn!("Failed to parse trust-lsp config: {err}");
                }
                return config;
            }
        };

        config.vendor_profile = super::normalize_optional_string(parsed.project.vendor_profile)
            .map(|profile| profile.to_ascii_lowercase());
        config.stdlib = parsed.project.stdlib.into();
        config.build = parsed.build.into();
        config.build.dependencies_offline |= options.offline_dependencies;
        let mut target_names = HashSet::new();
        config.targets = parsed
            .targets
            .into_iter()
            .map(super::TargetProfile::from)
            .filter(|target| {
                !target.name.is_empty() && target_names.insert(target.name.to_ascii_lowercase())
            })
            .collect();
        config.indexing = parsed.indexing.into();
        config.indexing.cache_enabled &= !options.no_index_cache;
        let diagnostics_section = parsed.diagnostics;
        config.diagnostic_external_paths =
            super::resolve_paths(root, &diagnostics_section.external_paths);
        config.diagnostics = super::DiagnosticSettings::from_config(
            config.vendor_profile.as_deref(),
            diagnostics_section,
        );
        config.runtime = parsed.runtime.into();
        config.workspace = super::WorkspaceSettings::from(parsed.workspace);
        config.telemetry = super::TelemetryConfig::from_section(root, parsed.telemetry);

        let mut include_paths = super::resolve_paths(root, &parsed.project.include_paths);
        config.include_paths.append(&mut include_paths);

        let mut libraries = Vec::new();
        for path in super::resolve_paths(root, &parsed.project.library_paths) {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("library")
                .to_string();
            libraries.push(super::LibrarySpec {
                name,
                path,
                version: None,
                dependencies: Vec::new(),
                docs: Vec::new(),
            });
        }
        for lib in parsed.libraries {
            let Some(path) = super::resolve_optional_path(root, &lib.path) else {
                continue;
            };
            let name = lib
                .name
                .clone()
                .or_else(|| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                })
                .unwrap_or_else(|| "library".to_string())
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let mut dependency_names = HashSet::new();
            let dependencies = lib
                .dependencies
                .into_iter()
                .map(super::LibraryDependency::from)
                .filter(|dependency| {
                    !dependency.name.is_empty()
                        && dependency_names.insert(dependency.name.to_ascii_lowercase())
                })
                .collect();
            let docs = super::resolve_paths(root, &lib.docs);
            libraries.push(super::LibrarySpec {
                name,
                path,
                version: super::normalize_optional_string(lib.version),
                dependencies,
                docs,
            });
        }
        let policy = super::DependencyPolicy::from(parsed.dependency_policy);
        let (dependencies, mut dependency_resolution_issues) =
            super::parse_project_dependencies(root, &parsed.dependencies);
        let (dependency_libraries, mut resolver_issues) =
            super::resolve_manifest_dependencies(root, &dependencies, &config.build, &policy);
        dependency_resolution_issues.append(&mut resolver_issues);
        libraries.extend(dependency_libraries);
        config.libraries = libraries;
        config.dependencies = dependencies;
        config.dependency_resolution_issues = dependency_resolution_issues;

        config
    }

    /// This configuration without what `options` rule out.
    fn limited_by(mut self, options: LoadOptions) -> Self {
        self.build.dependencies_offline |= options.offline_dependencies;
        self.indexing.cache_enabled &= !options.no_index_cache;
        self
    }

    fn base(root: &Path, config_path: Option<PathBuf>) -> Self {
        super::ProjectConfig {
            root: root.to_path_buf(),
            config_path,
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: super::StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: super::BuildConfig::default(),
            targets: Vec::new(),
            indexing: super::IndexingConfig::default(),
            diagnostics: super::DiagnosticSettings::default(),
            runtime: super::RuntimeConfig::default(),
            workspace: super::WorkspaceSettings::default(),
            telemetry: super::TelemetryConfig::default(),
        }
    }
}
