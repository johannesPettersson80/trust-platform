use smol_str::SmolStr;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::types::CompileError;
use super::super::util::{is_expression_kind, node_text};

#[derive(Debug, Clone, Copy)]
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

pub(super) fn var_block_qualifiers(node: &SyntaxNode) -> VarBlockQualifiers {
    let mut qualifiers = VarBlockQualifiers::default();
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
            SyntaxKind::KwRetain => qualifiers.retain = crate::RetainPolicy::Retain,
            SyntaxKind::KwNonRetain => qualifiers.retain = crate::RetainPolicy::NonRetain,
            SyntaxKind::KwPersistent => qualifiers.retain = crate::RetainPolicy::Persistent,
            SyntaxKind::KwConstant => qualifiers.constant = true,
            _ => {}
        }
    }
    qualifiers
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
