use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::symbols::{ParamDirection, Symbol, SymbolKind, SymbolTable};
use trust_hir::{Database, SourceDatabase, TypeId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use crate::util::{
    name_from_name_node, resolve_target_at_position_with_context, scope_at_position,
    ResolvedTarget,
};

/// Signature help result for a call site.
#[derive(Debug, Clone)]
pub struct SignatureHelpResult {
    /// All available signatures (usually a single entry).
    pub signatures: Vec<Signature>,
    /// The active signature index.
    pub active_signature: usize,
    /// The active parameter index.
    pub active_parameter: usize,
}

/// Parameter info for call signature metadata.
#[derive(Debug, Clone)]
pub struct CallSignatureParam {
    /// Parameter name.
    pub name: SmolStr,
    /// Parameter direction.
    pub direction: ParamDirection,
    /// Whether the parameter is declared CONSTANT.
    pub is_constant: bool,
}

/// Signature metadata for call transformation helpers.
#[derive(Debug, Clone)]
pub struct CallSignatureInfo {
    /// Callable name.
    pub name: SmolStr,
    /// Parameters in call order.
    pub params: Vec<CallSignatureParam>,
}

/// A callable signature.
#[derive(Debug, Clone)]
pub struct Signature {
    /// Display label for the signature.
    pub label: String,
    /// Parameter metadata for the signature.
    pub parameters: Vec<SignatureParameter>,
}

/// A single parameter in a signature.
#[derive(Debug, Clone)]
pub struct SignatureParameter {
    /// Display label for the parameter.
    pub label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ParamData {
    pub(crate) name: SmolStr,
    pub(crate) type_id: TypeId,
    pub(crate) direction: ParamDirection,
    pub(crate) is_constant: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SignatureInfo {
    pub(crate) name: SmolStr,
    pub(crate) params: Vec<ParamData>,
    pub(crate) return_type: Option<TypeId>,
}

#[derive(Debug, Clone)]
struct ArgInfo {
    name: Option<SmolStr>,
    range: TextRange,
}

pub(crate) struct CallSignatureContext {
    pub(crate) signature: SignatureInfo,
    pub(crate) used_params: FxHashSet<SmolStr>,
}

pub(crate) fn signature_for_call_expr(
    db: &Database,
    file_id: FileId,
    source: &str,
    root: &SyntaxNode,
    call_expr: &SyntaxNode,
) -> Option<SignatureInfo> {
    let arg_list = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)?;

    let symbols = db.file_symbols_with_project(file_id);
    let callee_offset = callee_name_offset(call_expr)?;
    let target =
        resolve_target_at_position_with_context(db, file_id, callee_offset, source, root, &symbols);

    let signature = match target {
        Some(ResolvedTarget::Symbol(symbol_id)) => {
            let symbol = symbols.get(symbol_id)?;
            signature_from_symbol(&symbols, symbol)
                .or_else(|| signature_from_type(&symbols, symbol.type_id))
        }
        Some(ResolvedTarget::Field(_)) => None,
        None => None,
    }
    .or_else(|| {
        let name = callee_name_text(call_expr)?;
        let scope_id = scope_at_position(&symbols, root, callee_offset);
        let symbol_id = symbols
            .resolve(name.as_str(), scope_id)
            .or_else(|| symbols.lookup(name.as_str()))?;
        let symbol = symbols.get(symbol_id)?;
        signature_from_symbol(&symbols, symbol)
            .or_else(|| signature_from_type(&symbols, symbol.type_id))
    })
    .or_else(|| {
        let name = callee_name_text(call_expr)?;
        let arg_count = collect_call_args(&arg_list).len();
        standard_signature(name.as_str(), arg_count)
    })?;

    Some(signature)
}

pub(crate) fn call_signature_context(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<CallSignatureContext> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let token = find_token_at_position(&root, position)?;
    let call_expr = token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::CallExpr)?;
    let arg_list = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)?;

    let signature = signature_for_call_expr(db, file_id, &source, &root, &call_expr)?;
    let args = collect_call_args(&arg_list);
    let arg_types = arg_types_for_args(db, file_id, &args);
    let signature = apply_arg_types(&signature, &arg_types);
    let formal_call = args.iter().any(|arg| arg.name.is_some());
    let signature = if formal_call {
        signature
    } else {
        strip_execution_params(&signature)
    };
    let mut used_params: FxHashSet<SmolStr> = FxHashSet::default();
    for arg in args {
        if let Some(name) = arg.name {
            used_params.insert(SmolStr::new(name.to_ascii_uppercase()));
        }
    }

    Some(CallSignatureContext {
        signature,
        used_params,
    })
}

/// Returns call signature metadata (name + parameters) for the call at position.
pub fn call_signature_info(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<CallSignatureInfo> {
    let context = call_signature_context(db, file_id, position)?;
    let params = context
        .signature
        .params
        .iter()
        .map(|param| CallSignatureParam {
            name: param.name.clone(),
            direction: param.direction,
            is_constant: param.is_constant,
        })
        .collect();
    Some(CallSignatureInfo {
        name: context.signature.name.clone(),
        params,
    })
}

/// Computes signature help information at a given position.
pub fn signature_help(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<SignatureHelpResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let token = find_token_at_position(&root, position)?;
    let call_expr = token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::CallExpr)?;
    let arg_list = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)?;
    let symbols = db.file_symbols_with_project(file_id);
    let signature = signature_for_call_expr(db, file_id, &source, &root, &call_expr)?;

    let args = collect_call_args(&arg_list);
    let arg_types = arg_types_for_args(db, file_id, &args);
    let signature = apply_arg_types(&signature, &arg_types);
    let formal_call = args.iter().any(|arg| arg.name.is_some());
    let signature = if formal_call {
        signature
    } else {
        strip_execution_params(&signature)
    };
    let active_arg = active_arg_index(&args, &arg_list, position);
    let mut active_param = active_param_index(&args, active_arg, &signature.params);
    if signature.params.is_empty() {
        active_param = 0;
    } else if active_param >= signature.params.len() {
        active_param = signature.params.len() - 1;
    }

    let label = format_signature_label(&symbols, &signature);
    let parameters = signature
        .params
        .iter()
        .map(|param| SignatureParameter {
            label: format_param_label(&symbols, param),
        })
        .collect();

    Some(SignatureHelpResult {
        signatures: vec![Signature { label, parameters }],
        active_signature: 0,
        active_parameter: active_param,
    })
}
