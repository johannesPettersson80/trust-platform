use super::helpers::qualify_name;
use super::*;

pub(super) fn collect_file_type_prelude(root: &SyntaxNode) -> FileTypePrelude {
    let mut entries = Vec::new();
    collect_type_prelude_entries(root, &mut Vec::new(), &mut entries);
    entries.sort_by(|left, right| left.qualified_name.cmp(&right.qualified_name));
    FileTypePrelude { entries }
}

fn collect_type_prelude_entries(
    node: &SyntaxNode,
    namespace: &mut Vec<SmolStr>,
    entries: &mut Vec<TypePreludeEntry>,
) {
    match node.kind() {
        SyntaxKind::Namespace => {
            let Some((parts, _)) = qualified_name_parts(node) else {
                return;
            };
            let pushed = parts.len();
            for (name, _) in parts {
                namespace.push(name);
            }
            for child in node.children() {
                collect_type_prelude_entries(&child, namespace, entries);
            }
            namespace.truncate(namespace.len().saturating_sub(pushed));
        }
        SyntaxKind::TypeDecl => {
            for (qualified_name, range, _type_def) in type_decl_entries(node, namespace) {
                entries.push(TypePreludeEntry {
                    qualified_name,
                    kind: ProjectTypeKind::Data,
                    range,
                });
            }
        }
        SyntaxKind::FunctionBlock => {
            if let Some((name, range)) = name_from_node(node) {
                entries.push(TypePreludeEntry {
                    qualified_name: qualify_name(namespace, &name),
                    kind: ProjectTypeKind::FunctionBlock,
                    range,
                });
            }
        }
        SyntaxKind::Class => {
            if let Some((name, range)) = name_from_node(node) {
                entries.push(TypePreludeEntry {
                    qualified_name: qualify_name(namespace, &name),
                    kind: ProjectTypeKind::Class,
                    range,
                });
            }
        }
        SyntaxKind::Interface => {
            if let Some((name, range)) = name_from_node(node) {
                entries.push(TypePreludeEntry {
                    qualified_name: qualify_name(namespace, &name),
                    kind: ProjectTypeKind::Interface,
                    range,
                });
            }
        }
        _ => {
            for child in node.children() {
                collect_type_prelude_entries(&child, namespace, entries);
            }
        }
    }
}

pub(super) fn build_project_type_catalog(
    files: &[(FileId, FileTypePrelude)],
) -> ProjectTypeCatalog {
    let mut entries = std::collections::BTreeMap::new();
    let mut duplicates = Vec::new();

    let mut ordered = files.to_vec();
    ordered.sort_by_key(|(file_id, _)| file_id.0);

    for (file_id, prelude) in ordered {
        for entry in prelude.entries {
            let candidate = ProjectTypeCatalogEntry {
                file_id,
                kind: entry.kind,
                range: entry.range,
            };
            if let Some(primary) = entries.get(entry.qualified_name.as_str()).cloned() {
                duplicates.push(ProjectTypeDuplicate {
                    qualified_name: entry.qualified_name.clone(),
                    primary,
                    duplicate: candidate,
                });
                continue;
            }
            entries.insert(entry.qualified_name, candidate);
        }
    }

    ProjectTypeCatalog {
        entries,
        duplicates,
    }
}

pub(super) fn find_project_type_declaration(
    root: &SyntaxNode,
    qualified_name: &str,
    kind: ProjectTypeKind,
) -> Option<SyntaxNode> {
    find_project_type_declaration_inner(root, &mut Vec::new(), qualified_name, kind)
}

fn find_project_type_declaration_inner(
    node: &SyntaxNode,
    namespace: &mut Vec<SmolStr>,
    qualified_name: &str,
    kind: ProjectTypeKind,
) -> Option<SyntaxNode> {
    match node.kind() {
        SyntaxKind::Namespace => {
            let (parts, _) = qualified_name_parts(node)?;
            let pushed = parts.len();
            for (name, _) in parts {
                namespace.push(name);
            }
            let found = node.children().find_map(|child| {
                find_project_type_declaration_inner(&child, namespace, qualified_name, kind)
            });
            namespace.truncate(namespace.len().saturating_sub(pushed));
            found
        }
        SyntaxKind::TypeDecl if matches!(kind, ProjectTypeKind::Data) => {
            type_decl_entries(node, namespace).into_iter().find_map(
                |(candidate, _range, type_def)| {
                    (candidate.as_str() == qualified_name).then_some(type_def)
                },
            )
        }
        SyntaxKind::FunctionBlock if matches!(kind, ProjectTypeKind::FunctionBlock) => {
            let (name, _) = name_from_node(node)?;
            (qualify_name(namespace, &name).as_str() == qualified_name).then(|| node.clone())
        }
        SyntaxKind::Class if matches!(kind, ProjectTypeKind::Class) => {
            let (name, _) = name_from_node(node)?;
            (qualify_name(namespace, &name).as_str() == qualified_name).then(|| node.clone())
        }
        SyntaxKind::Interface if matches!(kind, ProjectTypeKind::Interface) => {
            let (name, _) = name_from_node(node)?;
            (qualify_name(namespace, &name).as_str() == qualified_name).then(|| node.clone())
        }
        _ => node.children().find_map(|child| {
            find_project_type_declaration_inner(&child, namespace, qualified_name, kind)
        }),
    }
}

fn type_decl_entries(
    node: &SyntaxNode,
    namespace: &[SmolStr],
) -> Vec<(SmolStr, TextRange, SyntaxNode)> {
    let mut entries = Vec::new();
    let mut pending: Option<(SmolStr, TextRange)> = None;

    for child in node.children() {
        match child.kind() {
            SyntaxKind::Name => pending = name_from_node(&child),
            SyntaxKind::StructDef
            | SyntaxKind::UnionDef
            | SyntaxKind::EnumDef
            | SyntaxKind::ArrayType
            | SyntaxKind::TypeRef => {
                let Some((name, range)) = pending.take() else {
                    continue;
                };
                entries.push((qualify_name(namespace, &name), range, child));
            }
            _ => {}
        }
    }

    entries
}
