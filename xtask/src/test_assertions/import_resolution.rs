//! Visibility-aware Rust import resolution for assertion helper discovery.

use std::collections::{BTreeMap, BTreeSet};

use syn::{Item, UseTree, Visibility};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ImportVisibility {
    Private,
    Crate,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ImportPath {
    path: Vec<String>,
    visibility: ImportVisibility,
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
            import_visibility(&item_use.vis),
            &mut imports,
        );
    }
    imports
}

pub(super) fn candidate_paths(
    current_module: &[String],
    leading_colon: bool,
    segments: &[String],
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
            let imports = imports_by_module.get(current_module);
            if let Some(bindings) = imports.and_then(|imports| imports.bindings.get(first)) {
                for binding in bindings {
                    if import_is_visible(binding, current_module, current_module) {
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
                if let Some(imports) = imports {
                    for glob in &imports.glob_modules {
                        if import_is_visible(glob, current_module, current_module) {
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
    candidates
}

pub(super) fn definition_is_visible(
    visibility: &Visibility,
    declaring_module: &[String],
    requester: &[String],
) -> bool {
    match visibility {
        Visibility::Public(_) => true,
        Visibility::Restricted(restricted) if restricted.path.is_ident("crate") => true,
        Visibility::Inherited => requester.starts_with(declaring_module),
        Visibility::Restricted(_) => false,
    }
}

fn import_visibility(visibility: &Visibility) -> ImportVisibility {
    match visibility {
        Visibility::Public(_) => ImportVisibility::Crate,
        Visibility::Restricted(restricted) if restricted.path.is_ident("crate") => {
            ImportVisibility::Crate
        }
        Visibility::Inherited | Visibility::Restricted(_) => ImportVisibility::Private,
    }
}

fn collect_use_tree(
    tree: &UseTree,
    current_module: &[String],
    mut prefix: Vec<String>,
    anchored: bool,
    visibility: ImportVisibility,
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
            } else if segment != "self" {
                prefix.push(segment);
            }
            collect_use_tree(
                &path.tree,
                current_module,
                prefix,
                anchored,
                visibility,
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
                    visibility,
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
                    visibility,
                });
        }
        UseTree::Glob(_) => {
            imports.glob_modules.insert(ImportPath {
                path: prefix,
                visibility,
            });
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(
                    item,
                    current_module,
                    prefix.clone(),
                    anchored,
                    visibility,
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
        if import_is_visible(glob, module, requester) {
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

fn import_is_visible(
    import: &ImportPath,
    declaring_module: &[String],
    requester: &[String],
) -> bool {
    import.visibility == ImportVisibility::Crate || requester.starts_with(declaring_module)
}
