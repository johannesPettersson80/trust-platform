use smol_str::SmolStr;
use trust_hir::symbols::EdgeQualifier;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::types::CompileError;
use super::super::util::{is_expression_kind, node_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VarBlockKind {
    Input,
    Output,
    InOut,
    Var,
    Stat,
    Temp,
    Global,
    External,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct VarBlockQualifiers {
    pub(super) retain: crate::RetainPolicy,
    pub(super) constant: bool,
}

#[derive(Debug, Clone)]
pub(super) struct VarDeclParts {
    pub(super) names: Vec<SmolStr>,
    pub(super) type_ref: SyntaxNode,
    pub(super) initializer: Option<SyntaxNode>,
    pub(super) address: Option<SmolStr>,
}

pub(super) fn var_block_kind(node: &SyntaxNode) -> Result<VarBlockKind, CompileError> {
    for token in node
        .children_with_tokens()
        .filter_map(|child| child.into_token())
    {
        if token.kind().is_trivia() {
            continue;
        }
        return Ok(match token.kind() {
            SyntaxKind::KwVarInput => VarBlockKind::Input,
            SyntaxKind::KwVarOutput => VarBlockKind::Output,
            SyntaxKind::KwVarInOut => VarBlockKind::InOut,
            SyntaxKind::KwVarTemp => VarBlockKind::Temp,
            SyntaxKind::KwVar => VarBlockKind::Var,
            SyntaxKind::KwVarStat => VarBlockKind::Stat,
            SyntaxKind::KwVarGlobal => VarBlockKind::Global,
            SyntaxKind::KwVarExternal => VarBlockKind::External,
            _ => VarBlockKind::Unsupported,
        });
    }
    Err(CompileError::new("invalid VAR block"))
}

pub(super) fn var_block_qualifiers(node: &SyntaxNode) -> Result<VarBlockQualifiers, CompileError> {
    let mut qualifiers = VarBlockQualifiers::default();
    let mut qualifier_count = 0usize;
    for element in node.children_with_tokens() {
        if let Some(child) = element.as_node() {
            if child.kind() == SyntaxKind::VarDecl {
                break;
            }
        }
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        if token.kind().is_trivia() {
            continue;
        }
        match token.kind() {
            SyntaxKind::KwRetain => {
                qualifier_count += 1;
                qualifiers.retain = crate::RetainPolicy::Retain;
            }
            SyntaxKind::KwNonRetain => {
                qualifier_count += 1;
                qualifiers.retain = crate::RetainPolicy::NonRetain;
            }
            SyntaxKind::KwPersistent => {
                qualifier_count += 1;
                qualifiers.retain = crate::RetainPolicy::Persistent;
            }
            SyntaxKind::KwConstant => {
                qualifier_count += 1;
                qualifiers.constant = true;
            }
            _ => {}
        }
    }
    if qualifier_count > 1 {
        return Err(CompileError::new(
            "a variable section accepts at most one qualifier",
        ));
    }
    Ok(qualifiers)
}

pub(super) fn validate_retention_policy(
    owner: &str,
    kind: VarBlockKind,
    qualifiers: VarBlockQualifiers,
    retained_sections: &[VarBlockKind],
) -> Result<(), CompileError> {
    if qualifiers.retain != crate::RetainPolicy::Unspecified && !retained_sections.contains(&kind) {
        return Err(CompileError::new(format!(
            "retention qualifier is not allowed on this {owner} variable section"
        )));
    }
    Ok(())
}

pub(super) fn validate_special_var_sections(
    node: &SyntaxNode,
    owner: &str,
    allow_var_access: bool,
) -> Result<(), CompileError> {
    for child in node.children() {
        match child.kind() {
            SyntaxKind::VarAccessBlock if !allow_var_access => {
                return Err(CompileError::new(format!(
                    "VAR_ACCESS is not supported in {owner}"
                )));
            }
            SyntaxKind::VarConfigBlock => {
                return Err(CompileError::new(format!(
                    "VAR_CONFIG is not supported in {owner}"
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn parse_var_decl(var_decl: &SyntaxNode) -> Result<VarDeclParts, CompileError> {
    let mut names = Vec::new();
    for child in var_decl.children() {
        if child.kind() == SyntaxKind::Name {
            names.push(node_text(&child).into());
        }
    }
    if names.is_empty() {
        return Err(CompileError::new("missing variable name"));
    }

    let type_ref = var_decl
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .ok_or_else(|| CompileError::new("missing type in declaration"))?;

    let initializer = var_decl
        .children()
        .find(|child| is_expression_kind(child.kind()));

    let mut address = None;
    let mut seen_at = false;
    for element in var_decl.children_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::KwAt => seen_at = true,
            SyntaxKind::DirectAddress if seen_at => {
                address = Some(SmolStr::new(token.text()));
                seen_at = false;
            }
            _ if !token.kind().is_trivia() => seen_at = false,
            _ => {}
        }
    }

    Ok(VarDeclParts {
        names,
        type_ref,
        initializer,
        address,
    })
}

pub(super) fn edge_qualifier_from_decl(node: &SyntaxNode) -> Option<EdgeQualifier> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find_map(|token| match token.kind() {
            SyntaxKind::KwREdge => Some(EdgeQualifier::Rising),
            SyntaxKind::KwFEdge => Some(EdgeQualifier::Falling),
            _ => None,
        })
}

pub(super) fn reject_borrowed_storage_initializer(
    kind: VarBlockKind,
    parts: &VarDeclParts,
) -> Result<(), CompileError> {
    if parts.initializer.is_none() {
        return Ok(());
    }
    match kind {
        VarBlockKind::InOut => Err(CompileError::new(
            "VAR_IN_OUT declarations cannot have an initializer",
        )),
        VarBlockKind::External => Err(CompileError::new(
            "VAR_EXTERNAL declarations cannot have an initializer",
        )),
        _ => Ok(()),
    }
}
