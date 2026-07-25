use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use quote::ToTokens;
use serde::Serialize;
use syn::{Attribute, Item, LitStr};

use super::cfg_policy::is_test_only;
use super::model::{ModuleGraph, RustSourceUnit};

pub fn scan_module_graph(root: &Path, source_root: &Path, module: &str) -> Result<ModuleGraph> {
    scan_module_graph_with_directory(root, source_root, module, None).map(|result| result.0)
}

pub fn scan_crate_root_module_graph(
    root: &Path,
    source_root: &Path,
    module: &str,
) -> Result<ModuleGraph> {
    scan_module_graph_with_directory(
        root,
        source_root,
        module,
        source_root.parent().map(Path::to_path_buf),
    )
    .map(|result| result.0)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SourceTopologyBinding {
    pub path: String,
    pub module: String,
    pub topology_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceTopologyScan {
    pub schema_version: u32,
    pub selected_path: String,
    pub bindings: Vec<SourceTopologyBinding>,
    pub diagnostics: Vec<String>,
}

#[cfg(test)]
pub fn scan_source_topology(
    root: &Path,
    source_root: &Path,
    module: &str,
    selected_path: &Path,
) -> Result<SourceTopologyScan> {
    let mut scans = scan_source_topologies(root, source_root, module, &[selected_path])?;
    Ok(scans.remove(0))
}

pub fn scan_source_topologies(
    root: &Path,
    source_root: &Path,
    module: &str,
    selected_paths: &[&Path],
) -> Result<Vec<SourceTopologyScan>> {
    let canonical_root = root.canonicalize().context("canonicalize code-spec root")?;
    let mut selected_relatives = Vec::new();
    for selected_path in selected_paths {
        let selected = selected_path.canonicalize().with_context(|| {
            format!(
                "canonicalize selected Rust source {}",
                selected_path.display()
            )
        })?;
        if !selected.starts_with(&canonical_root) {
            anyhow::bail!(
                "selected Rust source escapes repository root: {}",
                selected_path.display()
            )
        }
        selected_relatives.push(display_path(root, &selected));
    }
    selected_relatives.sort();
    selected_relatives.dedup();
    if selected_relatives.len() != selected_paths.len() {
        anyhow::bail!("selected Rust topology paths must be unique")
    }
    let (graph, topology) = scan_module_graph_with_directory(root, source_root, module, None)?;
    Ok(selected_relatives
        .into_iter()
        .map(|selected_relative| {
            let mut bindings = topology
                .iter()
                .filter(|binding| binding.path == selected_relative)
                .cloned()
                .collect::<Vec<_>>();
            bindings.sort();
            bindings.dedup();
            let diagnostics = if bindings.is_empty() {
                graph.diagnostics.clone()
            } else {
                Vec::new()
            };
            SourceTopologyScan {
                schema_version: 1,
                selected_path: selected_relative,
                bindings,
                diagnostics,
            }
        })
        .collect())
}

fn scan_module_graph_with_directory(
    root: &Path,
    source_root: &Path,
    module: &str,
    module_dir: Option<PathBuf>,
) -> Result<(ModuleGraph, Vec<SourceTopologyBinding>)> {
    let canonical_root = root.canonicalize().context("canonicalize code-spec root")?;
    let mut scanner = ModuleScanner {
        root,
        canonical_root,
        visited: BTreeSet::new(),
        sources: Vec::new(),
        test_only_sources: BTreeSet::new(),
        diagnostics: Vec::new(),
        topology: Vec::new(),
    };
    scanner.scan_file(source_root, module, module_dir, false, Vec::new());
    scanner
        .sources
        .sort_by(|left, right| (&left.path, &left.module).cmp(&(&right.path, &right.module)));
    scanner.diagnostics.sort();
    scanner.diagnostics.dedup();
    let production_paths = scanner
        .sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<BTreeSet<_>>();
    let test_only_sources = scanner
        .test_only_sources
        .into_iter()
        .filter(|path| !production_paths.contains(path))
        .collect();
    scanner.topology.sort();
    scanner.topology.dedup();
    Ok((
        ModuleGraph {
            sources: scanner.sources,
            test_only_sources,
            diagnostics: scanner.diagnostics,
        },
        scanner.topology,
    ))
}

struct ModuleScanner<'a> {
    root: &'a Path,
    canonical_root: PathBuf,
    visited: BTreeSet<(PathBuf, String, bool)>,
    sources: Vec<RustSourceUnit>,
    test_only_sources: BTreeSet<String>,
    diagnostics: Vec<String>,
    topology: Vec<SourceTopologyBinding>,
}

impl ModuleScanner<'_> {
    fn scan_file(
        &mut self,
        path: &Path,
        module: &str,
        inherited_module_dir: Option<PathBuf>,
        test_only: bool,
        mut topology_paths: Vec<String>,
    ) {
        let canonical = match path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                if !test_only {
                    self.diagnostics.push(format!(
                        "unreadable module source {}: {error}",
                        display_path(self.root, path)
                    ));
                }
                return;
            }
        };
        if !canonical.starts_with(&self.canonical_root) {
            self.diagnostics.push(format!(
                "module source escapes repository root: {}",
                path.display()
            ));
            return;
        }
        let relative = display_path(self.root, &canonical);
        if topology_paths.last() != Some(&relative) {
            topology_paths.push(relative.clone());
        }
        if !test_only {
            self.topology.push(SourceTopologyBinding {
                path: relative.clone(),
                module: module.to_string(),
                topology_paths: topology_paths.clone(),
            });
        }
        let key = (canonical.clone(), module.to_string(), test_only);
        if !self.visited.insert(key) {
            return;
        }
        if test_only {
            self.test_only_sources.insert(relative.clone());
        } else {
            self.sources.push(RustSourceUnit {
                path: relative.clone(),
                module: module.to_string(),
            });
        }
        let source = match fs::read_to_string(&canonical) {
            Ok(source) => source,
            Err(error) => {
                if !test_only {
                    self.diagnostics
                        .push(format!("unreadable Rust source {relative}: {error}"));
                }
                return;
            }
        };
        let file = match syn::parse_file(&source) {
            Ok(file) => file,
            Err(error) => {
                if !test_only {
                    self.diagnostics
                        .push(format!("unsupported Rust syntax in {relative}: {error}"));
                }
                return;
            }
        };
        let module_dir = inherited_module_dir.unwrap_or_else(|| module_directory(&canonical));
        self.scan_items(
            &canonical,
            &module_dir,
            module,
            &file.items,
            test_only,
            &topology_paths,
        );
    }

    fn scan_items(
        &mut self,
        current_file: &Path,
        module_dir: &Path,
        module: &str,
        items: &[Item],
        test_only: bool,
        topology_paths: &[String],
    ) {
        for item in items {
            let item_test_only = test_only || is_test_only(item_attrs(item));
            match item {
                Item::Mod(item_mod) => {
                    let child_module = format!("{module}::{}", item_mod.ident);
                    if let Some((_, inline_items)) = &item_mod.content {
                        let inline_dir = module_dir.join(item_mod.ident.to_string());
                        self.scan_items(
                            current_file,
                            &inline_dir,
                            &child_module,
                            inline_items,
                            item_test_only,
                            topology_paths,
                        );
                    } else {
                        self.scan_external_module(
                            current_file,
                            module_dir,
                            &child_module,
                            &item_mod.attrs,
                            &item_mod.ident.to_string(),
                            item_test_only,
                            topology_paths,
                        );
                    }
                }
                Item::Macro(item_macro) if item_macro.mac.path.is_ident("include") => {
                    self.scan_include(
                        current_file,
                        module_dir,
                        module,
                        item_macro,
                        item_test_only,
                        topology_paths,
                    );
                }
                _ => {}
            }
        }
    }

    fn scan_external_module(
        &mut self,
        current_file: &Path,
        module_dir: &Path,
        child_module: &str,
        attributes: &[Attribute],
        name: &str,
        test_only: bool,
        topology_paths: &[String],
    ) {
        if let Some(explicit) = path_attribute(attributes) {
            let explicit_base = current_file.parent().unwrap_or(module_dir);
            let explicit_path = explicit_base.join(explicit);
            let explicit_module_dir = explicit_path.parent().map(Path::to_path_buf);
            self.scan_file(
                &explicit_path,
                child_module,
                explicit_module_dir,
                test_only,
                topology_paths.to_vec(),
            );
            return;
        }
        let candidates = [
            module_dir.join(format!("{name}.rs")),
            module_dir.join(name).join("mod.rs"),
        ];
        let existing = candidates
            .iter()
            .filter(|candidate| candidate.is_file())
            .collect::<Vec<_>>();
        match existing.as_slice() {
            [path] => self.scan_file(path, child_module, None, test_only, topology_paths.to_vec()),
            [] if !test_only => self.diagnostics.push(format!(
                "unresolved module {child_module} from {}",
                display_path(self.root, current_file)
            )),
            _ if !test_only => self.diagnostics.push(format!(
                "multiply-resolved module {child_module} from {}",
                display_path(self.root, current_file)
            )),
            _ => {}
        }
    }

    fn scan_include(
        &mut self,
        current_file: &Path,
        module_dir: &Path,
        module: &str,
        item: &syn::ItemMacro,
        test_only: bool,
        topology_paths: &[String],
    ) {
        match syn::parse2::<LitStr>(item.mac.tokens.clone()) {
            Ok(path) => {
                let Some(parent) = current_file.parent() else {
                    self.diagnostics.push(format!(
                        "include! source has no parent: {}",
                        current_file.display()
                    ));
                    return;
                };
                self.scan_file(
                    &parent.join(path.value()),
                    module,
                    Some(module_dir.to_path_buf()),
                    test_only,
                    topology_paths.to_vec(),
                );
            }
            Err(_) if !test_only => self.diagnostics.push(format!(
                "unsupported include! expression in {}: {}",
                display_path(self.root, current_file),
                item.mac.tokens.to_token_stream()
            )),
            Err(_) => {}
        }
    }
}

fn module_directory(source_file: &Path) -> PathBuf {
    let parent = source_file.parent().unwrap_or_else(|| Path::new(""));
    match source_file.file_name().and_then(|name| name.to_str()) {
        Some("lib.rs" | "main.rs" | "mod.rs" | "build.rs") => parent.to_path_buf(),
        _ => parent.join(
            source_file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default(),
        ),
    }
}

fn path_attribute(attributes: &[Attribute]) -> Option<String> {
    attributes.iter().find_map(|attribute| {
        if !attribute.path().is_ident("path") {
            return None;
        }
        let syn::Meta::NameValue(name_value) = &attribute.meta else {
            return None;
        };
        let syn::Expr::Lit(expression) = &name_value.value else {
            return None;
        };
        let syn::Lit::Str(value) = &expression.lit else {
            return None;
        };
        Some(value.value())
    })
}

fn item_attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        _ => &[],
    }
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
