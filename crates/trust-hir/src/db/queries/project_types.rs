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
        SyntaxKind::StmtList | SyntaxKind::VarBlock => {}
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
            // ST identifiers are case-insensitive: vendor stubs spell a
            // qualified type one way (`WagoTypesLog.Fb_...`) while project
            // code writes another (`...FB_...`). Key the catalog by the
            // same normalized form the symbol table uses
            // (`normalize_name` = ASCII uppercase).
            let key = SmolStr::new(entry.qualified_name.to_ascii_uppercase());
            if let Some(primary) = entries.get(key.as_str()).cloned() {
                duplicates.push(ProjectTypeDuplicate {
                    qualified_name: entry.qualified_name.clone(),
                    primary,
                    duplicate: candidate,
                });
                continue;
            }
            entries.insert(key, candidate);
        }
    }

    ProjectTypeCatalog {
        entries,
        duplicates,
    }
}

/// Resolve the declaration at the name range already recorded by the project catalog.
/// The catalog and syntax belong to the same Salsa source revision. Searching other
/// declarations here repeats a whole library walk for every imported type.
pub(super) fn find_project_type_declaration(
    root: &SyntaxNode,
    qualified_name: &str,
    kind: ProjectTypeKind,
    range: TextRange,
) -> Option<SyntaxNode> {
    let element = root.covering_element(range);
    let node = match element {
        rowan::NodeOrToken::Node(node) => node,
        rowan::NodeOrToken::Token(token) => token.parent()?,
    };
    let expected = match kind {
        ProjectTypeKind::Data => SyntaxKind::TypeDecl,
        ProjectTypeKind::FunctionBlock => SyntaxKind::FunctionBlock,
        ProjectTypeKind::Class => SyntaxKind::Class,
        ProjectTypeKind::Interface => SyntaxKind::Interface,
    };
    let declaration = node.ancestors().find(|node| node.kind() == expected)?;
    let mut namespaces: Vec<_> = declaration
        .ancestors()
        .filter(|node| node.kind() == SyntaxKind::Namespace)
        .collect();
    namespaces.reverse();
    let namespace: Vec<_> = namespaces
        .iter()
        .flat_map(|node| {
            qualified_name_parts(node)
                .into_iter()
                .flat_map(|(parts, _)| parts.into_iter().map(|(name, _)| name))
        })
        .collect();
    if kind == ProjectTypeKind::Data {
        type_decl_entries(&declaration, &namespace)
            .into_iter()
            .find_map(|(name, found_range, definition)| {
                (found_range == range && name.eq_ignore_ascii_case(qualified_name))
                    .then_some(definition)
            })
    } else {
        let (name, found_range) = name_from_node(&declaration)?;
        (found_range == range
            && qualify_name(&namespace, &name).eq_ignore_ascii_case(qualified_name))
        .then_some(declaration)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ranges_resolve_each_kind_nested_namespaces_and_multiple_types() {
        let text = "NAMESPACE Outer.Inner
TYPE A : INT; B : STRUCT X : INT; END_STRUCT END_TYPE
FUNCTION_BLOCK Device VAR X : INT; END_VAR X := X + 1; END_FUNCTION_BLOCK
CLASS Thing END_CLASS
INTERFACE Capability END_INTERFACE
END_NAMESPACE
NAMESPACE Other TYPE A : BOOL; END_TYPE END_NAMESPACE";
        let parse = trust_syntax::parser::parse(text);
        assert!(parse.ok(), "{:?}", parse.errors());
        let root = parse.syntax();
        let prelude = collect_file_type_prelude(&root);
        assert_eq!(prelude.entries.len(), 6);
        for entry in &prelude.entries {
            let declaration = find_project_type_declaration(
                &root,
                &entry.qualified_name,
                entry.kind,
                entry.range,
            )
            .expect("every catalog entry resolves at its recorded source location");
            assert!(
                declaration.text_range().contains_range(entry.range)
                    || entry.kind == ProjectTypeKind::Data
            );
            assert!(
                find_project_type_declaration(&root, "Wrong.Name", entry.kind, entry.range,)
                    .is_none()
            );
        }
        let other = prelude
            .entries
            .iter()
            .find(|entry| entry.qualified_name == "Other.A")
            .unwrap();
        assert_eq!(
            find_project_type_declaration(&root, "Other.A", other.kind, other.range)
                .unwrap()
                .text()
                .to_string()
                .trim(),
            "BOOL"
        );
    }
}
