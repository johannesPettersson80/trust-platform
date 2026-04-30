use super::const_utils::*;
use super::*;
use crate::db::diagnostics::{is_expression_kind, is_pou_kind};

impl SymbolCollector<'_> {
    pub(super) fn precollect_pous(&mut self, node: &SyntaxNode, namespace: &[SmolStr]) {
        let mut current_ns: Vec<SmolStr> = namespace.to_vec();
        if node.kind() == SyntaxKind::Namespace {
            if let Some((parts, _)) = qualified_name_parts(node) {
                for (name, _) in parts {
                    current_ns.push(name);
                }
            }
        }

        // Pre-register FB/CLASS/INTERFACE types before main pass
        match node.kind() {
            SyntaxKind::FunctionBlock => {
                if let Some((name, _)) = name_from_node(node) {
                    let qualified = qualify_name(&current_ns, &name);
                    self.table
                        .register_type(qualified.clone(), Type::FunctionBlock { name: qualified });
                }
            }
            SyntaxKind::Class => {
                if let Some((name, _)) = name_from_node(node) {
                    let qualified = qualify_name(&current_ns, &name);
                    self.table
                        .register_type(qualified.clone(), Type::Class { name: qualified });
                }
            }
            SyntaxKind::Interface => {
                if let Some((name, _)) = name_from_node(node) {
                    let qualified = qualify_name(&current_ns, &name);
                    self.table
                        .register_type(qualified.clone(), Type::Interface { name: qualified });
                }
            }
            _ => {}
        }

        for child in node.children() {
            self.precollect_pous(&child, &current_ns);
        }
    }

    pub(super) fn precollect_types(&mut self, node: &SyntaxNode, namespace: &[SmolStr]) {
        let mut current_ns: Vec<SmolStr> = namespace.to_vec();
        if node.kind() == SyntaxKind::Namespace {
            if let Some((parts, _)) = qualified_name_parts(node) {
                for (name, _) in parts {
                    current_ns.push(name);
                }
            }
        }

        if node.kind() == SyntaxKind::TypeDecl {
            self.register_type_names(node, &current_ns);
        }
        for child in node.children() {
            self.precollect_types(&child, &current_ns);
        }
    }

    pub(super) fn precollect_constants(
        &mut self,
        node: &SyntaxNode,
        namespace: &[SmolStr],
        pou_stack: &[SmolStr],
    ) {
        let mut current_namespace: Vec<SmolStr> = namespace.to_vec();
        let mut current_pou_stack: Vec<SmolStr> = pou_stack.to_vec();

        if node.kind() == SyntaxKind::Namespace {
            if let Some((parts, _)) = qualified_name_parts(node) {
                current_namespace.extend(parts.into_iter().map(|(name, _)| name));
            }
        } else if is_pou_kind(node.kind()) {
            current_pou_stack.extend(pou_scope_parts(node));
        }

        if node.kind() == SyntaxKind::VarBlock && const_block_is_precollectable(node) {
            let current_scope = const_scope_identity(&current_namespace, &current_pou_stack);
            self.collect_const_block(node, &current_scope);
        }

        for child in node.children() {
            self.precollect_constants(&child, &current_namespace, &current_pou_stack);
        }
    }

    pub(super) fn collect_const_block(&mut self, node: &SyntaxNode, scope: &Option<SmolStr>) {
        for var_decl in node.children().filter(|n| n.kind() == SyntaxKind::VarDecl) {
            let expr = var_decl
                .children()
                .find(|child| is_expression_kind(child.kind()));
            let Some(expr) = expr else {
                continue;
            };

            for name_node in var_decl.children().filter(|n| n.kind() == SyntaxKind::Name) {
                if let Some((name, _)) = name_from_node(&name_node) {
                    let key = const_key(scope, name.as_str());
                    self.const_exprs.entry(key).or_insert(expr.clone());
                }
            }
        }
    }
}

fn const_block_is_precollectable(node: &SyntaxNode) -> bool {
    if !var_block_is_constant(node) {
        return false;
    }

    match var_qualifier_from_block(node) {
        VarQualifier::Global | VarQualifier::External => true,
        VarQualifier::Local => !node
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .any(|token| {
                matches!(
                    token.kind(),
                    SyntaxKind::KwVarAccess | SyntaxKind::KwVarConfig
                )
            }),
        VarQualifier::Input
        | VarQualifier::Output
        | VarQualifier::InOut
        | VarQualifier::Temp
        | VarQualifier::Static
        | VarQualifier::Access => false,
    }
}
