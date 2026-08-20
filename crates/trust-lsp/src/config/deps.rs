use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub(super) fn parse_project_dependencies(
    root: &Path,
    entries: &BTreeMap<String, super::ManifestDependencyEntry>,
) -> (
    Vec<super::ProjectDependency>,
    Vec<super::DependencyResolutionIssue>,
) {
    let mut dependencies = Vec::new();
    let mut issues = Vec::new();
    for (name, entry) in entries {
        match parse_project_dependency(root, name, entry) {
            Ok(dependency) => dependencies.push(dependency),
            Err(message) => issues.push(super::DependencyResolutionIssue {
                code: "L005",
                dependency: name.clone(),
                message,
            }),
        }
    }
    dependencies.sort_by_key(|dependency| dependency.name.to_ascii_lowercase());
    (dependencies, issues)
}

pub(super) fn resolve_manifest_dependencies(
    root: &Path,
    dependencies: &[super::ProjectDependency],
    build: &super::BuildConfig,
    policy: &super::DependencyPolicy,
) -> (
    Vec<super::LibrarySpec>,
    Vec<super::DependencyResolutionIssue>,
) {
    let issues = Vec::new();
    let lock_path = super::dependency_lock_path(root, build);
    let lock = match super::load_dependency_lock(&lock_path) {
        Ok(lock) => lock,
        Err(message) => {
            return (
                Vec::new(),
                vec![super::DependencyResolutionIssue {
                    code: "L006",
                    dependency: "lockfile".to_string(),
                    message,
                }],
            );
        }
    };

    let mut resolver = DependencyResolver::new(root, build, policy, &lock, issues);
    resolver.resolve_all(dependencies);
    let (libraries, mut issues, resolved_lock) = resolver.finish();

    if issues.is_empty() && !build.dependencies_locked && !resolved_lock.is_empty() {
        if let Err(message) = super::write_dependency_lock(&lock_path, resolved_lock) {
            issues.push(super::DependencyResolutionIssue {
                code: "L006",
                dependency: "lockfile".to_string(),
                message,
            });
        }
    }

    (libraries.into_values().collect(), issues)
}

struct DependencyResolver<'a> {
    root: &'a Path,
    build: &'a super::BuildConfig,
    policy: &'a super::DependencyPolicy,
    lock: &'a super::DependencyLockFile,
    states: HashMap<String, DependencyVisitState>,
    libraries: BTreeMap<String, super::LibrarySpec>,
    issues: Vec<super::DependencyResolutionIssue>,
    resolved_lock: BTreeMap<String, super::DependencyLockEntry>,
    stack: Vec<String>,
}

impl<'a> DependencyResolver<'a> {
    fn new(
        root: &'a Path,
        build: &'a super::BuildConfig,
        policy: &'a super::DependencyPolicy,
        lock: &'a super::DependencyLockFile,
        issues: Vec<super::DependencyResolutionIssue>,
    ) -> Self {
        Self {
            root,
            build,
            policy,
            lock,
            states: HashMap::new(),
            libraries: BTreeMap::new(),
            issues,
            resolved_lock: BTreeMap::new(),
            stack: Vec::new(),
        }
    }

    fn resolve_all(&mut self, dependencies: &[super::ProjectDependency]) {
        for dependency in dependencies {
            self.resolve_dependency_recursive(dependency);
        }
    }

    fn finish(
        self,
    ) -> (
        BTreeMap<String, super::LibrarySpec>,
        Vec<super::DependencyResolutionIssue>,
        BTreeMap<String, super::DependencyLockEntry>,
    ) {
        (self.libraries, self.issues, self.resolved_lock)
    }

    fn resolve_dependency_recursive(&mut self, dependency: &super::ProjectDependency) {
        let key = canonical_dependency_name(&dependency.name);
        let path = match resolve_dependency_source(
            self.root,
            self.build,
            self.policy,
            self.lock,
            dependency,
            &mut self.resolved_lock,
        ) {
            Ok(path) => path,
            Err(issue) => {
                self.issues.push(issue);
                return;
            }
        };
        if !path.is_dir() {
            self.issues.push(super::DependencyResolutionIssue {
                code: "L001",
                dependency: dependency.name.clone(),
                message: format!(
                    "Dependency '{}' path does not exist: {}",
                    dependency.name,
                    path.display()
                ),
            });
            return;
        }

        if self
            .states
            .get(&key)
            .copied()
            .is_some_and(|state| state == DependencyVisitState::Visiting)
        {
            let cycle_start = self
                .stack
                .iter()
                .position(|name| canonical_dependency_name(name) == key)
                .unwrap_or(0);
            let mut cycle = self.stack[cycle_start..].to_vec();
            cycle.push(dependency.name.clone());
            self.issues.push(super::DependencyResolutionIssue {
                code: "L004",
                dependency: dependency.name.clone(),
                message: format!("Dependency cycle detected: {}", cycle.join(" -> ")),
            });
            return;
        }

        if let Some(existing) = self.libraries.get(&key) {
            if existing.path != path {
                self.issues.push(super::DependencyResolutionIssue {
                    code: "L003",
                    dependency: dependency.name.clone(),
                    message: format!(
                        "Dependency '{}' resolves to conflicting sources: {} and {}",
                        dependency.name,
                        existing.path.display(),
                        path.display()
                    ),
                });
                return;
            }
            if let Some(required) = dependency.version.as_deref() {
                if existing.version.as_deref() != Some(required) {
                    let available = existing.version.as_deref().unwrap_or("unspecified");
                    self.issues.push(super::DependencyResolutionIssue {
                        code: "L002",
                        dependency: dependency.name.clone(),
                        message: format!(
                            "Dependency '{}' requested version {}, but resolved version is {}",
                            dependency.name, required, available
                        ),
                    });
                }
            }
            return;
        }

        self.states
            .insert(key.clone(), DependencyVisitState::Visiting);
        self.stack.push(dependency.name.clone());

        let (package, nested_dependencies) = match load_dependency_manifest(&path) {
            Ok(manifest) => {
                let (nested, mut parse_issues) =
                    parse_project_dependencies(&path, &manifest.dependencies);
                self.issues.append(&mut parse_issues);
                (manifest.package, nested)
            }
            Err(message) => {
                self.issues.push(super::DependencyResolutionIssue {
                    code: "L001",
                    dependency: dependency.name.clone(),
                    message,
                });
                self.stack.pop();
                self.states.remove(&key);
                return;
            }
        };

        if let Some(required) = dependency.version.as_deref() {
            if package.version.as_deref() != Some(required) {
                let available = package.version.as_deref().unwrap_or("unspecified");
                self.issues.push(super::DependencyResolutionIssue {
                    code: "L002",
                    dependency: dependency.name.clone(),
                    message: format!(
                        "Dependency '{}' requested version {}, but resolved package version is {}",
                        dependency.name, required, available
                    ),
                });
            }
        }

        let mut library_dependencies = Vec::new();
        for nested in &nested_dependencies {
            library_dependencies.push(super::LibraryDependency {
                name: nested.name.clone(),
                version: nested.version.clone(),
            });
            self.resolve_dependency_recursive(nested);
        }

        self.libraries.insert(
            key.clone(),
            super::LibrarySpec {
                name: dependency.name.clone(),
                path,
                version: package.version,
                dependencies: library_dependencies,
                docs: Vec::new(),
            },
        );
        self.stack.pop();
        self.states.insert(key, DependencyVisitState::Done);
    }
}

fn canonical_dependency_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn parse_project_dependency(
    root: &Path,
    name: &str,
    entry: &super::ManifestDependencyEntry,
) -> Result<super::ProjectDependency, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Dependency name must not be blank".to_string());
    }
    match entry {
        super::ManifestDependencyEntry::Path(path) => {
            let path = super::resolve_optional_path(root, path)
                .ok_or_else(|| format!("Dependency '{name}' path must not be blank"))?;
            Ok(super::ProjectDependency {
                name: name.to_string(),
                path: Some(path),
                git: None,
                version: None,
            })
        }
        super::ManifestDependencyEntry::Detailed(section) => {
            for (field, value) in [
                ("version", &section.version),
                ("rev", &section.rev),
                ("tag", &section.tag),
                ("branch", &section.branch),
            ] {
                if value.as_ref().is_some_and(|value| value.trim().is_empty()) {
                    return Err(format!("Dependency '{name}' `{field}` must not be blank"));
                }
            }
            let has_path = section
                .path
                .as_ref()
                .is_some_and(|path| !path.trim().is_empty());
            let has_git = section
                .git
                .as_ref()
                .is_some_and(|git| !git.trim().is_empty());

            if has_path == has_git {
                return Err(format!(
                    "Dependency '{name}' must set exactly one of `path` or `git`"
                ));
            }

            let rev = super::normalize_optional_string(section.rev.clone());
            let tag = super::normalize_optional_string(section.tag.clone());
            let branch = super::normalize_optional_string(section.branch.clone());
            let version = super::normalize_optional_string(section.version.clone());
            let selector_count = usize::from(rev.is_some())
                + usize::from(tag.is_some())
                + usize::from(branch.is_some());
            if selector_count > 1 {
                return Err(format!(
                    "Dependency '{name}' may set only one of `rev`, `tag`, or `branch`"
                ));
            }

            if has_path {
                if rev.is_some() || tag.is_some() || branch.is_some() {
                    return Err(format!(
                        "Dependency '{name}' path entries do not support `rev`, `tag`, or `branch`"
                    ));
                }
                let path = section.path.as_deref().unwrap_or_default();
                let path = super::resolve_optional_path(root, path)
                    .ok_or_else(|| format!("Dependency '{name}' path must not be blank"))?;
                return Ok(super::ProjectDependency {
                    name: name.to_string(),
                    path: Some(path),
                    git: None,
                    version,
                });
            }

            Ok(super::ProjectDependency {
                name: name.to_string(),
                path: None,
                git: Some(super::GitDependency {
                    url: section.git.clone().unwrap_or_default().trim().to_string(),
                    rev,
                    tag,
                    branch,
                }),
                version,
            })
        }
    }
}

fn resolve_dependency_source(
    root: &Path,
    build: &super::BuildConfig,
    policy: &super::DependencyPolicy,
    lock: &super::DependencyLockFile,
    dependency: &super::ProjectDependency,
    resolved_lock: &mut BTreeMap<String, super::DependencyLockEntry>,
) -> Result<PathBuf, super::DependencyResolutionIssue> {
    if let Some(path) = dependency.path.as_ref() {
        let resolved = canonicalize_or_self(path);
        if build.dependencies_locked {
            match find_lock_entry(lock, &dependency.name) {
                Some(super::DependencyLockEntry::Path { path })
                    if Path::new(path) == resolved.as_path() => {}
                Some(super::DependencyLockEntry::Path { .. }) => {
                    return Err(super::DependencyResolutionIssue {
                        code: "L006",
                        dependency: dependency.name.clone(),
                        message: format!(
                            "Dependency '{}' lock entry path does not match canonical source",
                            dependency.name
                        ),
                    });
                }
                Some(super::DependencyLockEntry::Git { .. }) => {
                    return Err(super::DependencyResolutionIssue {
                        code: "L006",
                        dependency: dependency.name.clone(),
                        message: format!(
                            "Dependency '{}' lock entry source kind does not match local path",
                            dependency.name
                        ),
                    });
                }
                None => {
                    return Err(super::DependencyResolutionIssue {
                        code: "L006",
                        dependency: dependency.name.clone(),
                        message: format!(
                            "Dependency '{}' has no lock entry in locked mode",
                            dependency.name
                        ),
                    });
                }
            }
        }
        resolved_lock.insert(
            dependency.name.clone(),
            super::DependencyLockEntry::Path {
                path: resolved.to_string_lossy().into_owned(),
            },
        );
        return Ok(resolved);
    }

    let Some(git) = dependency.git.as_ref() else {
        return Err(super::DependencyResolutionIssue {
            code: "L005",
            dependency: dependency.name.clone(),
            message: format!("Dependency '{}' has no source", dependency.name),
        });
    };

    let resolved = resolve_git_dependency(root, build, policy, lock, &dependency.name, git)?;
    resolved_lock.insert(
        dependency.name.clone(),
        super::DependencyLockEntry::Git {
            url: git.url.clone(),
            rev: resolved.rev.clone(),
        },
    );
    Ok(resolved.path)
}

fn find_lock_entry<'a>(
    lock: &'a super::DependencyLockFile,
    name: &str,
) -> Option<&'a super::DependencyLockEntry> {
    lock.dependencies
        .iter()
        .find(|(entry_name, _)| entry_name.eq_ignore_ascii_case(name))
        .map(|(_, entry)| entry)
}

fn resolve_git_dependency(
    root: &Path,
    build: &super::BuildConfig,
    policy: &super::DependencyPolicy,
    lock: &super::DependencyLockFile,
    dependency_name: &str,
    git: &super::GitDependency,
) -> Result<super::ResolvedGitDependency, super::DependencyResolutionIssue> {
    if let Err(message) = super::validate_git_source_policy(git.url.as_str(), policy) {
        return Err(super::DependencyResolutionIssue {
            code: "L005",
            dependency: dependency_name.to_string(),
            message: format!("Dependency '{dependency_name}' rejected by trust policy: {message}"),
        });
    }

    let lock_entry = find_lock_entry(lock, dependency_name);
    let selector = match (git.rev.as_ref(), git.tag.as_ref(), git.branch.as_ref()) {
        (Some(rev), None, None) => super::RevisionSelector::Rev(rev.clone()),
        (None, Some(tag), None) => super::RevisionSelector::Tag(tag.clone()),
        (None, None, Some(branch)) => super::RevisionSelector::Branch(branch.clone()),
        (None, None, None) => {
            if build.dependencies_locked {
                match lock_entry {
                    Some(super::DependencyLockEntry::Git { url, rev })
                        if url.trim() == git.url.trim() =>
                    {
                        super::RevisionSelector::Rev(rev.clone())
                    }
                    Some(super::DependencyLockEntry::Git { .. }) => {
                        return Err(super::DependencyResolutionIssue {
                            code: "L006",
                            dependency: dependency_name.to_string(),
                            message: format!(
                                "Dependency '{dependency_name}' lock entry URL mismatch for locked resolution"
                            ),
                        });
                    }
                    _ => {
                        return Err(super::DependencyResolutionIssue {
                            code: "L006",
                            dependency: dependency_name.to_string(),
                            message: format!(
                                "Dependency '{dependency_name}' requires `rev`/`tag`/`branch` or lock entry in locked mode"
                            ),
                        });
                    }
                }
            } else if let Some(super::DependencyLockEntry::Git { url, rev }) = lock_entry {
                if url.trim() == git.url.trim() {
                    super::RevisionSelector::Rev(rev.clone())
                } else {
                    super::RevisionSelector::DefaultHead
                }
            } else {
                super::RevisionSelector::DefaultHead
            }
        }
        _ => {
            return Err(super::DependencyResolutionIssue {
                code: "L005",
                dependency: dependency_name.to_string(),
                message: format!(
                    "Dependency '{dependency_name}' may set only one of `rev`, `tag`, or `branch`"
                ),
            });
        }
    };

    let repo_root = root.join(".trust-lsp").join("deps").join("git");
    let repo_dir = repo_root.join(format!(
        "{}-{}",
        super::sanitize_for_path(dependency_name),
        super::stable_hash_hex(git.url.as_str())
    ));

    if !repo_dir.is_dir() {
        if build.dependencies_offline {
            return Err(super::DependencyResolutionIssue {
                code: "L007",
                dependency: dependency_name.to_string(),
                message: format!(
                    "Dependency '{dependency_name}' is not available in offline mode (missing cache at {})",
                    repo_dir.display()
                ),
            });
        }
        std::fs::create_dir_all(&repo_root).map_err(|err| super::DependencyResolutionIssue {
            code: "L001",
            dependency: dependency_name.to_string(),
            message: format!(
                "Dependency '{dependency_name}' failed to create git cache root: {err}"
            ),
        })?;
        super::run_git_command(
            None,
            &[
                "clone",
                "--no-checkout",
                git.url.as_str(),
                repo_dir.to_string_lossy().as_ref(),
            ],
        )
        .map_err(|message| super::DependencyResolutionIssue {
            code: "L001",
            dependency: dependency_name.to_string(),
            message: format!("Dependency '{dependency_name}' clone failed: {message}"),
        })?;
    } else if !build.dependencies_offline {
        super::run_git_command(Some(&repo_dir), &["fetch", "--tags", "--prune", "origin"])
            .map_err(|message| super::DependencyResolutionIssue {
                code: "L001",
                dependency: dependency_name.to_string(),
                message: format!("Dependency '{dependency_name}' fetch failed: {message}"),
            })?;
    }

    let resolved_rev = super::resolve_git_revision(&repo_dir, &selector).ok_or_else(|| {
        let detail = match selector {
            super::RevisionSelector::Rev(rev) => format!("rev {rev}"),
            super::RevisionSelector::Tag(tag) => format!("tag {tag}"),
            super::RevisionSelector::Branch(branch) => format!("branch {branch}"),
            super::RevisionSelector::DefaultHead => "default HEAD".to_string(),
        };
        let code = if build.dependencies_offline {
            "L007"
        } else {
            "L001"
        };
        super::DependencyResolutionIssue {
            code,
            dependency: dependency_name.to_string(),
            message: format!(
                "Dependency '{dependency_name}' could not resolve git {detail} in {}",
                repo_dir.display()
            ),
        }
    })?;

    super::run_git_command(
        Some(&repo_dir),
        &["checkout", "--detach", "--force", resolved_rev.as_str()],
    )
    .map_err(|message| super::DependencyResolutionIssue {
        code: "L001",
        dependency: dependency_name.to_string(),
        message: format!("Dependency '{dependency_name}' checkout failed: {message}"),
    })?;

    Ok(super::ResolvedGitDependency {
        path: repo_dir,
        rev: resolved_rev,
    })
}

fn load_dependency_manifest(path: &Path) -> Result<super::DependencyManifestFile, String> {
    let Some(config_path) = super::find_config_file(path) else {
        return Ok(super::DependencyManifestFile::default());
    };
    let contents = std::fs::read_to_string(&config_path).map_err(|err| {
        format!(
            "Failed to read dependency manifest for '{}': {} ({err})",
            path.display(),
            config_path.display()
        )
    })?;
    toml::from_str(&contents).map_err(|err| {
        format!(
            "Failed to parse dependency manifest for '{}': {err}",
            path.display()
        )
    })
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyVisitState {
    Visiting,
    Done,
}
