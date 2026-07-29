//! Visibility-aware Rust import resolution for assertion helper discovery.

use std::collections::{BTreeMap, BTreeSet};

use syn::{Item, UseTree, Visibility};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ImportPath {
    path: Vec<String>,
    visible_from: Vec<String>,
}

#[derive(Clone, Default)]
pub(super) struct ImportReachability {
    glob_modules: BTreeSet<ImportPath>,
    bindings: BTreeMap<String, BTreeSet<ImportPath>>,
}

impl ImportReachability {
    pub(super) fn merge(&mut self, other: Self) {
        self.glob_modules.extend(other.glob_modules);
        for (name, bindings) in other.bindings {
            self.bindings.entry(name).or_default().extend(bindings);
        }
    }
}

pub(super) fn imports_from_items(items: &[Item], current_module: &[String]) -> ImportReachability {
    let mut imports = ImportReachability::default();
    for item in items {
        let Item::Use(item_use) = item else {
            continue;
        };
        collect_use_tree(
            &item_use.tree,
            current_module,
            Vec::new(),
            false,
            import_visibility(&item_use.vis, current_module),
            &mut imports,
        );
    }
    imports
}

pub(super) fn candidate_paths(
    current_module: &[String],
    leading_colon: bool,
    segments: &[String],
    local_imports: &ImportReachability,
    imports_by_module: &BTreeMap<Vec<String>, ImportReachability>,
) -> BTreeSet<Vec<String>> {
    if segments.is_empty() {
        return BTreeSet::new();
    }
    let mut candidates = BTreeSet::new();
    if leading_colon {
        let mut absolute = segments.to_vec();
        if absolute.first().map(String::as_str) == Some("crate") {
            absolute.remove(0);
        }
        candidates.insert(absolute);
        return candidates;
    }
    match segments[0].as_str() {
        "crate" => {
            candidates.insert(segments[1..].to_vec());
        }
        "self" => {
            let mut path = current_module.to_vec();
            path.extend_from_slice(&segments[1..]);
            candidates.insert(path);
        }
        "super" => {
            let mut path = current_module.to_vec();
            let mut index = 0;
            while segments.get(index).map(String::as_str) == Some("super") {
                path.pop();
                index += 1;
            }
            path.extend_from_slice(&segments[index..]);
            candidates.insert(path);
        }
        first => {
            let module_imports = imports_by_module.get(current_module);
            for bindings in local_imports.bindings.get(first).into_iter().chain(
                module_imports
                    .and_then(|imports| imports.bindings.get(first))
                    .into_iter(),
            ) {
                for binding in bindings {
                    if import_is_visible(binding, current_module) {
                        let mut path = binding.path.clone();
                        path.extend_from_slice(&segments[1..]);
                        candidates.insert(path);
                    }
                }
            }
            if segments.len() == 1 {
                let mut local = current_module.to_vec();
                local.push(first.to_string());
                candidates.insert(local);
                for imports in std::iter::once(local_imports).chain(module_imports.into_iter()) {
                    for glob in &imports.glob_modules {
                        if import_is_visible(glob, current_module) {
                            collect_glob_candidates(
                                current_module,
                                &glob.path,
                                first,
                                imports_by_module,
                                &mut BTreeSet::new(),
                                &mut candidates,
                            );
                        }
                    }
                }
            } else {
                let mut relative = current_module.to_vec();
                relative.extend_from_slice(segments);
                candidates.insert(relative);
                candidates.insert(segments.to_vec());
            }
        }
    }
    let initial = candidates;
    let mut expanded = BTreeSet::new();
    for candidate in initial {
        collect_named_import_candidates(
            current_module,
            &candidate,
            imports_by_module,
            &mut BTreeSet::new(),
            &mut expanded,
        );
    }
    expanded
}

fn collect_named_import_candidates(
    requester: &[String],
    candidate: &[String],
    imports_by_module: &BTreeMap<Vec<String>, ImportReachability>,
    visited: &mut BTreeSet<Vec<String>>,
    candidates: &mut BTreeSet<Vec<String>>,
) {
    if !visited.insert(candidate.to_vec()) {
        return;
    }
    candidates.insert(candidate.to_vec());
    let Some((name, declaring_module)) = candidate.split_last() else {
        return;
    };
    let Some(imports) = imports_by_module.get(declaring_module) else {
        return;
    };
    if let Some(bindings) = imports.bindings.get(name) {
        for binding in bindings {
            if import_is_visible(binding, requester) {
                collect_named_import_candidates(
                    requester,
                    &binding.path,
                    imports_by_module,
                    visited,
                    candidates,
                );
            }
        }
    }
    for glob in &imports.glob_modules {
        if import_is_visible(glob, requester) {
            let mut imported = glob.path.clone();
            imported.push(name.clone());
            collect_named_import_candidates(
                requester,
                &imported,
                imports_by_module,
                visited,
                candidates,
            );
        }
    }
}

pub(super) fn definition_is_visible(
    visibility: &Visibility,
    declaring_module: &[String],
    requester: &[String],
) -> bool {
    match visibility {
        Visibility::Public(_) => true,
        Visibility::Inherited => requester.starts_with(declaring_module),
        Visibility::Restricted(restricted) => {
            restricted_visibility_scope(&restricted.path, declaring_module)
                .is_some_and(|scope| requester.starts_with(&scope))
        }
    }
}

fn restricted_visibility_scope(
    path: &syn::Path,
    declaring_module: &[String],
) -> Option<Vec<String>> {
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let first = segments.first()?.as_str();
    let mut index = 1;
    let mut scope = match first {
        "crate" => Vec::new(),
        "self" => declaring_module.to_vec(),
        "super" => {
            let mut scope = declaring_module.to_vec();
            scope.pop()?;
            while segments.get(index).map(String::as_str) == Some("super") {
                scope.pop()?;
                index += 1;
            }
            scope
        }
        _ => return None,
    };
    scope.extend_from_slice(&segments[index..]);
    Some(scope)
}

fn import_visibility(visibility: &Visibility, declaring_module: &[String]) -> Vec<String> {
    match visibility {
        Visibility::Public(_) => Vec::new(),
        Visibility::Inherited => declaring_module.to_vec(),
        Visibility::Restricted(restricted) => {
            restricted_visibility_scope(&restricted.path, declaring_module)
                .unwrap_or_else(|| declaring_module.to_vec())
        }
    }
}

fn collect_use_tree(
    tree: &UseTree,
    current_module: &[String],
    mut prefix: Vec<String>,
    anchored: bool,
    visible_from: Vec<String>,
    imports: &mut ImportReachability,
) {
    match tree {
        UseTree::Path(path) => {
            let segment = path.ident.to_string();
            let mut anchored = anchored;
            if prefix.is_empty() && !anchored {
                match segment.as_str() {
                    "crate" => anchored = true,
                    "self" => {
                        prefix.extend_from_slice(current_module);
                        anchored = true;
                    }
                    "super" => {
                        prefix.extend_from_slice(
                            &current_module[..current_module.len().saturating_sub(1)],
                        );
                        anchored = true;
                    }
                    _ => {
                        prefix.extend_from_slice(current_module);
                        prefix.push(segment);
                        anchored = true;
                    }
                }
            } else if segment == "super" {
                prefix.pop();
            } else if segment != "self" {
                prefix.push(segment);
            }
            collect_use_tree(
                &path.tree,
                current_module,
                prefix,
                anchored,
                visible_from,
                imports,
            );
        }
        UseTree::Name(name) => {
            let imported = name.ident.to_string();
            if prefix.is_empty() && !anchored {
                prefix.extend_from_slice(current_module);
            }
            prefix.push(imported.clone());
            imports
                .bindings
                .entry(imported)
                .or_default()
                .insert(ImportPath {
                    path: prefix,
                    visible_from,
                });
        }
        UseTree::Rename(rename) => {
            if prefix.is_empty() && !anchored {
                prefix.extend_from_slice(current_module);
            }
            prefix.push(rename.ident.to_string());
            imports
                .bindings
                .entry(rename.rename.to_string())
                .or_default()
                .insert(ImportPath {
                    path: prefix,
                    visible_from,
                });
        }
        UseTree::Glob(_) => {
            imports.glob_modules.insert(ImportPath {
                path: prefix,
                visible_from,
            });
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(
                    item,
                    current_module,
                    prefix.clone(),
                    anchored,
                    visible_from.clone(),
                    imports,
                );
            }
        }
    }
}

fn collect_glob_candidates(
    requester: &[String],
    module: &[String],
    name: &str,
    imports_by_module: &BTreeMap<Vec<String>, ImportReachability>,
    visited: &mut BTreeSet<Vec<String>>,
    candidates: &mut BTreeSet<Vec<String>>,
) {
    if !visited.insert(module.to_vec()) {
        return;
    }
    let mut direct = module.to_vec();
    direct.push(name.to_string());
    candidates.insert(direct);
    let Some(imports) = imports_by_module.get(module) else {
        return;
    };
    for glob in &imports.glob_modules {
        if import_is_visible(glob, requester) {
            collect_glob_candidates(
                requester,
                &glob.path,
                name,
                imports_by_module,
                visited,
                candidates,
            );
        }
    }
}

fn import_is_visible(import: &ImportPath, requester: &[String]) -> bool {
    requester.starts_with(&import.visible_from)
}
