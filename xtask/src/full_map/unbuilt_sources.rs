//! `FULLMAP-UNBUILT`: every Rust file in the repository belongs to a build.
//!
//! A file leaves the build without any failure when the `mod` line that
//! declared it is removed: it stops compiling and its tests stop running. The
//! same-computer ADS identity check was lost this way in `dd476a78f` (owner,
//! 16 October 2026: add a check so that this cannot happen again).
//!
//! Starting at every build target of every package, the check follows rustc's
//! module rules: `mod` declarations with and without `#[path]`, inline modules,
//! and `include!`, whose file declares its modules relative to its own folder.
//! A module whose file is missing is one that `cfg` removes; rustc never loads
//! it, so it is not reported. Vendored code under `third_party/` is not ours
//! and is skipped.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use proc_macro2::{TokenStream, TokenTree};
use serde::Deserialize;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};

use super::{rel_path, FullMapCheck, PackageSummary};

pub(super) const CHECK_ID: &str = "FULLMAP-UNBUILT";
const SUMMARY: &str = "every Rust file belongs to a build";

/// A Rust file the policy accepts outside every build, with its owner and reason.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct UnbuiltSourceAllowance {
    pub(super) path: String,
    pub(super) owner: String,
    pub(super) reason: String,
}

/// Checks the workspace members reported by `cargo metadata` and every other
/// package in the repository.
pub(super) fn check_unbuilt_sources(
    root: &Path,
    members: &[PackageSummary],
    allowances: &[UnbuiltSourceAllowance],
) -> FullMapCheck {
    match unbuilt_source_check(root, members, allowances) {
        Ok(check) => check,
        Err(err) => FullMapCheck::fail(CHECK_ID, SUMMARY, vec![format!("{err:#}")]),
    }
}

fn unbuilt_source_check(
    root: &Path,
    members: &[PackageSummary],
    allowances: &[UnbuiltSourceAllowance],
) -> Result<FullMapCheck> {
    let packages = repository_packages(root, members)?;
    let package_dirs = packages
        .iter()
        .map(|package| package.dir.clone())
        .collect::<BTreeSet<_>>();
    let mut unbuilt = BTreeSet::new();
    let mut failures = Vec::new();
    let mut file_count = 0;
    for package in &packages {
        let mut walk = ModuleWalk::new(root, &package.dir);
        for target in &package.targets {
            walk.visit_file(target, &parent_dir(target), None);
        }
        let mut files = Vec::new();
        package_rust_files(&package.dir, &package.dir, &package_dirs, &mut files)?;
        file_count += files.len();
        for file in files {
            if !walk.reached.contains(&file) {
                unbuilt.insert(rel_path(root, &file));
            }
        }
        failures.extend(walk.problems);
    }

    let allowed = allowances
        .iter()
        .map(|allowance| (allowance.path.as_str(), allowance))
        .collect::<BTreeMap<_, _>>();
    for path in &unbuilt {
        if !allowed.contains_key(path.as_str()) {
            failures.push(format!(
                "{path} is not part of any build: declare it with `mod`, delete it, or give it an owner and reason in unbuilt_source_allowlist"
            ));
        }
    }
    for allowance in allowances {
        if allowance.owner.trim().is_empty() || allowance.reason.trim().is_empty() {
            failures.push(format!(
                "{}: an unbuilt_source_allowlist entry needs an owner and a reason",
                allowance.path
            ));
        }
        if !unbuilt.contains(&allowance.path) {
            let state = if root.join(&allowance.path).is_file() {
                "is built again"
            } else {
                "no longer exists"
            };
            failures.push(format!(
                "{} {state}; remove its unbuilt_source_allowlist entry",
                allowance.path
            ));
        }
    }

    if !failures.is_empty() {
        return Ok(FullMapCheck::fail(CHECK_ID, SUMMARY, failures));
    }
    let mut details = vec![format!(
        "{file_count} Rust files in {} packages; every file not listed below is built",
        packages.len()
    )];
    details.extend(allowances.iter().map(|allowance| {
        format!(
            "accepted outside the build: {} ({})",
            allowance.path, allowance.owner
        )
    }));
    Ok(FullMapCheck::pass(CHECK_ID, SUMMARY, details))
}

/// A package folder and the main files of its build targets.
struct Package {
    dir: PathBuf,
    targets: Vec<PathBuf>,
}

/// Workspace members come from `cargo metadata`; packages outside the workspace,
/// such as the fuzz and firmware packages, are read from their manifests.
fn repository_packages(root: &Path, members: &[PackageSummary]) -> Result<Vec<Package>> {
    let mut packages = Vec::new();
    for member in members {
        let dir = parent_dir(&root.join(&member.manifest_path));
        let targets = member
            .targets
            .iter()
            .map(|target| normalize(&root.join(&target.src_path)))
            .collect();
        packages.push(Package { dir, targets });
    }
    let member_dirs = packages
        .iter()
        .map(|package| package.dir.clone())
        .collect::<BTreeSet<_>>();
    let mut manifests = Vec::new();
    find_manifests(root, root, &mut manifests)?;
    for manifest in manifests {
        let dir = parent_dir(&manifest);
        if member_dirs.contains(&dir) || dir == normalize(root) {
            continue;
        }
        let targets = manifest_targets(&dir)?;
        packages.push(Package { dir, targets });
    }
    Ok(packages)
}

fn find_manifests(root: &Path, dir: &Path, manifests: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            if skipped_dir(&path) || path == root.join("third_party") {
                continue;
            }
            find_manifests(root, &path, manifests)?;
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            manifests.push(normalize(&path));
        }
    }
    Ok(())
}

/// The targets cargo builds for a manifest: the library, the build script, the
/// declared targets, and the automatic ones unless `auto*` turns them off.
fn manifest_targets(dir: &Path) -> Result<Vec<PathBuf>> {
    let manifest_path = dir.join("Cargo.toml");
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let manifest: toml::Value =
        toml::from_str(&text).with_context(|| format!("parse {}", manifest_path.display()))?;
    let package = manifest.get("package");
    let automatic = |key: &str| {
        package
            .and_then(|package| package.get(key))
            .and_then(toml::Value::as_bool)
            .unwrap_or(true)
    };

    let mut targets = Vec::new();
    match manifest
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
    {
        Some(path) => targets.push(dir.join(path)),
        None => targets.push(dir.join("src/lib.rs")),
    }
    match package.and_then(|package| package.get("build")) {
        Some(toml::Value::String(path)) => targets.push(dir.join(path)),
        Some(toml::Value::Boolean(false)) => {}
        _ => targets.push(dir.join("build.rs")),
    }
    for (section, folder) in [
        ("bin", "src/bin"),
        ("test", "tests"),
        ("bench", "benches"),
        ("example", "examples"),
    ] {
        let declared = manifest.get(section).and_then(toml::Value::as_array);
        for target in declared.into_iter().flatten() {
            if let Some(path) = target.get("path").and_then(toml::Value::as_str) {
                targets.push(dir.join(path));
            } else if let Some(name) = target.get("name").and_then(toml::Value::as_str) {
                targets.push(dir.join(folder).join(format!("{name}.rs")));
                targets.push(dir.join(folder).join(name).join("main.rs"));
                if section == "bin" {
                    targets.push(dir.join("src/main.rs"));
                }
            }
        }
    }
    if automatic("autobins") {
        targets.push(dir.join("src/main.rs"));
    }
    for (key, folder) in [
        ("autobins", "src/bin"),
        ("autotests", "tests"),
        ("autobenches", "benches"),
        ("autoexamples", "examples"),
    ] {
        if !automatic(key) {
            continue;
        }
        let folder = dir.join(folder);
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                targets.push(path.join("main.rs"));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                targets.push(path);
            }
        }
    }
    targets.retain(|path| path.is_file());
    Ok(targets.iter().map(|path| normalize(path)).collect())
}

/// Every `.rs` file of one package, leaving out nested packages and build output.
fn package_rust_files(
    package_dir: &Path,
    dir: &Path,
    package_dirs: &BTreeSet<PathBuf>,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
        let path = normalize(&entry?.path());
        if path.is_dir() {
            if skipped_dir(&path) || (path != package_dir && package_dirs.contains(&path)) {
                continue;
            }
            package_rust_files(package_dir, &path, package_dirs, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn skipped_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "target" || name == "node_modules" || name.starts_with('.'))
}

/// Follows the module tree from target files, as rustc loads it.
struct ModuleWalk<'a> {
    root: &'a Path,
    package_dir: &'a Path,
    reached: BTreeSet<PathBuf>,
    visited: BTreeSet<(PathBuf, PathBuf, Option<String>)>,
    problems: Vec<String>,
}

impl<'a> ModuleWalk<'a> {
    fn new(root: &'a Path, package_dir: &'a Path) -> Self {
        Self {
            root,
            package_dir,
            reached: BTreeSet::new(),
            visited: BTreeSet::new(),
            problems: Vec::new(),
        }
    }

    /// Visits one module file. `dir` and `relative` follow rustc's directory
    /// ownership: a crate root, `mod.rs`, `#[path]` or included file declares its
    /// child modules in `dir`; a `name.rs` file declares them in `dir/name`.
    fn visit_file(&mut self, file: &Path, dir: &Path, relative: Option<&str>) {
        let file = normalize(file);
        if !file.is_file() {
            return;
        }
        let key = (file.clone(), normalize(dir), relative.map(str::to_owned));
        if !self.visited.insert(key) {
            return;
        }
        self.reached.insert(file.clone());
        let parsed = match fs::read_to_string(&file)
            .map_err(anyhow::Error::from)
            .and_then(|source| syn::parse_file(&source).map_err(anyhow::Error::from))
        {
            Ok(parsed) => parsed,
            Err(err) => {
                self.problems.push(format!(
                    "{}: not read as Rust ({err}); the modules it declares were not followed",
                    rel_path(self.root, &file)
                ));
                return;
            }
        };
        self.visit_items(&parsed.items, dir, relative);

        let mut includes = IncludeCollector::default();
        includes.visit_file(&parsed);
        let file_dir = parent_dir(&file);
        for parts in includes.invocations {
            if let Some(target) = include_target(self.package_dir, &file_dir, &parts) {
                let target_dir = parent_dir(&target);
                self.visit_file(&target, &target_dir, None);
            }
        }
    }

    fn visit_items(&mut self, items: &[syn::Item], dir: &Path, relative: Option<&str>) {
        for item in items {
            let syn::Item::Mod(module) = item else {
                continue;
            };
            let name = module.ident.unraw().to_string();
            match (&module.content, path_attribute(&module.attrs)) {
                (Some((_, items)), Some(path)) => self.visit_items(items, &dir.join(path), None),
                (Some((_, items)), None) => {
                    self.visit_items(items, &child_dir(dir, relative).join(&name), None);
                }
                (None, Some(path)) => {
                    let target = dir.join(path);
                    self.visit_file(&target, &parent_dir(&target), None);
                }
                (None, None) => {
                    let base = child_dir(dir, relative);
                    let flat = base.join(format!("{name}.rs"));
                    if flat.is_file() {
                        self.visit_file(&flat, &base, Some(&name));
                    } else {
                        let nested = base.join(&name).join("mod.rs");
                        self.visit_file(&nested, &base.join(&name), None);
                    }
                }
            }
        }
    }
}

fn path_attribute(attributes: &[syn::Attribute]) -> Option<String> {
    attributes.iter().find_map(|attribute| {
        let syn::Meta::NameValue(pair) = &attribute.meta else {
            return None;
        };
        if !pair.path.is_ident("path") {
            return None;
        }
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(text),
            ..
        }) = &pair.value
        else {
            return None;
        };
        Some(text.value())
    })
}

/// One piece of an `include!` argument: literal text or an `env!` variable.
#[derive(Debug, Clone, PartialEq, Eq)]
enum IncludePart {
    Environment(String),
    Text(String),
}

#[derive(Default)]
struct IncludeCollector {
    invocations: Vec<Vec<IncludePart>>,
}

impl<'ast> Visit<'ast> for IncludeCollector {
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node.path.is_ident("include") {
            let mut parts = Vec::new();
            include_parts(node.tokens.clone(), &mut parts);
            self.invocations.push(parts);
        }
        visit::visit_macro(self, node);
    }
}

fn include_parts(tokens: TokenStream, parts: &mut Vec<IncludePart>) {
    let mut trees = tokens.into_iter();
    while let Some(tree) = trees.next() {
        match tree {
            TokenTree::Ident(ident) if ident == "env" => {
                let _bang = trees.next();
                if let Some(TokenTree::Group(group)) = trees.next() {
                    let mut names = Vec::new();
                    include_parts(group.stream(), &mut names);
                    if let Some(IncludePart::Text(name)) = names.into_iter().next() {
                        parts.push(IncludePart::Environment(name));
                    }
                }
            }
            TokenTree::Group(group) => include_parts(group.stream(), parts),
            TokenTree::Literal(literal) => {
                if let Ok(text) = syn::parse2::<syn::LitStr>(TokenTree::Literal(literal).into()) {
                    parts.push(IncludePart::Text(text.value()));
                }
            }
            TokenTree::Ident(_) | TokenTree::Punct(_) => {}
        }
    }
}

/// `include!("x.rs")` is relative to the including file, and
/// `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/x.rs"))` to the package.
/// Other variables, such as `OUT_DIR`, name generated files outside the repository.
fn include_target(package_dir: &Path, file_dir: &Path, parts: &[IncludePart]) -> Option<PathBuf> {
    let (base, rest, from_package) = match parts.split_first()? {
        (IncludePart::Environment(name), rest) if name == "CARGO_MANIFEST_DIR" => {
            (package_dir, rest, true)
        }
        (IncludePart::Environment(_), _) => return None,
        (IncludePart::Text(_), _) => (file_dir, parts, false),
    };
    let mut path = String::new();
    for part in rest {
        match part {
            IncludePart::Text(text) => path.push_str(text),
            IncludePart::Environment(_) => return None,
        }
    }
    if path.is_empty() {
        return None;
    }
    let relative = if from_package {
        path.trim_start_matches('/')
    } else {
        path.as_str()
    };
    Some(base.join(relative))
}

fn child_dir(dir: &Path, relative: Option<&str>) -> PathBuf {
    relative.map_or_else(|| dir.to_path_buf(), |name| dir.join(name))
}

fn parent_dir(path: &Path) -> PathBuf {
    normalize(path.parent().unwrap_or(Path::new("")))
}

/// Resolves `.` and `..` without touching the file system, so symbolic links do
/// not change paths.
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
#[path = "unbuilt_sources/tests.rs"]
mod tests;
