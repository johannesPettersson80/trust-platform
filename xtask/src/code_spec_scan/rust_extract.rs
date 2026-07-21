use std::collections::BTreeSet;

use anyhow::{Context, Result};
use proc_macro2::Span;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Attribute, ExprAssign, ExprCall, ExprMacro, ExprMethodCall, ExprUnsafe, ImplItem, Item,
    TraitItem, Type, Visibility,
};

use super::cfg_policy::is_test_only;
use super::model::RustFact;
use super::RustScan;

pub fn extract_rust_source(module: &str, source: &str) -> Result<RustScan> {
    let file = syn::parse_file(source).context("parse Rust source")?;
    let mut facts = Vec::new();
    scan_items(module, &file.items, source, &[], &mut facts);
    facts.sort_by(|left, right| {
        (&left.qualified_symbol, &left.item_kind, left.body_start).cmp(&(
            &right.qualified_symbol,
            &right.item_kind,
            right.body_start,
        ))
    });
    Ok(RustScan {
        schema_version: 1,
        facts,
        diagnostics: Vec::new(),
    })
}

fn scan_items(
    module: &str,
    items: &[Item],
    source: &str,
    inherited_cfg: &[String],
    facts: &mut Vec<RustFact>,
) {
    let mut fuzz_target_ordinal = 0usize;
    for item in items {
        if is_test_only(item_attrs(item)) {
            continue;
        }
        match item {
            Item::Fn(function) => {
                let symbol = qualify(module, &function.sig.ident.to_string());
                let fact_index = push_fact(
                    facts,
                    source,
                    symbol.clone(),
                    "rust_function",
                    visibility(&function.vis),
                    signature_with_cfg(
                        inherited_cfg,
                        &function.attrs,
                        format!(
                            "{} {}",
                            function.vis.to_token_stream(),
                            function.sig.to_token_stream()
                        ),
                    ),
                    function.span(),
                );
                let risks = scan_expressions(
                    &symbol,
                    &function.block,
                    source,
                    inherited_cfg,
                    &function.attrs,
                    facts,
                );
                set_fact_risks(facts, fact_index, risks);
            }
            Item::Impl(item_impl) => {
                let self_type = qualified_self_type(module, &item_impl.self_ty);
                let trait_name = item_impl
                    .trait_
                    .as_ref()
                    .map(|(_, path, _)| compact_tokens(path));
                for impl_item in &item_impl.items {
                    if let ImplItem::Fn(method) = impl_item {
                        if is_test_only(&method.attrs) {
                            continue;
                        }
                        let owner = trait_name.as_ref().map_or_else(
                            || self_type.clone(),
                            |trait_name| format!("<{self_type} as {trait_name}>"),
                        );
                        let symbol = format!("{owner}::{}", method.sig.ident);
                        let fact_index = push_fact(
                            facts,
                            source,
                            symbol.clone(),
                            "rust_method",
                            visibility(&method.vis),
                            signature_with_cfg(
                                inherited_cfg,
                                &method.attrs,
                                format!(
                                    "{} {}",
                                    method.vis.to_token_stream(),
                                    method.sig.to_token_stream()
                                ),
                            ),
                            method.span(),
                        );
                        let risks = scan_expressions(
                            &symbol,
                            &method.block,
                            source,
                            inherited_cfg,
                            &method.attrs,
                            facts,
                        );
                        set_fact_risks(facts, fact_index, risks);
                    }
                }
            }
            Item::Trait(item_trait) => {
                let trait_name = qualify(module, &item_trait.ident.to_string());
                for trait_item in &item_trait.items {
                    if let TraitItem::Fn(method) = trait_item {
                        if is_test_only(&method.attrs) {
                            continue;
                        }
                        let symbol = format!("{trait_name}::{}", method.sig.ident);
                        let fact_index = push_fact(
                            facts,
                            source,
                            symbol.clone(),
                            "rust_trait_method",
                            "public",
                            signature_with_cfg(
                                inherited_cfg,
                                &method.attrs,
                                method.sig.to_token_stream().to_string(),
                            ),
                            method.span(),
                        );
                        if let Some(block) = &method.default {
                            let risks = scan_expressions(
                                &symbol,
                                block,
                                source,
                                inherited_cfg,
                                &method.attrs,
                                facts,
                            );
                            set_fact_risks(facts, fact_index, risks);
                        }
                    }
                }
            }
            Item::Struct(item_struct) if is_public(&item_struct.vis) => {
                let _ = push_fact(
                    facts,
                    source,
                    qualify(module, &item_struct.ident.to_string()),
                    "rust_public_type",
                    "public",
                    signature_with_cfg(
                        inherited_cfg,
                        &item_struct.attrs,
                        format!(
                            "{} struct {}",
                            item_struct.vis.to_token_stream(),
                            item_struct.ident
                        ),
                    ),
                    item_struct.span(),
                );
            }
            Item::Enum(item_enum) if is_public(&item_enum.vis) => {
                let _ = push_fact(
                    facts,
                    source,
                    qualify(module, &item_enum.ident.to_string()),
                    "rust_public_type",
                    "public",
                    signature_with_cfg(
                        inherited_cfg,
                        &item_enum.attrs,
                        format!(
                            "{} enum {}",
                            item_enum.vis.to_token_stream(),
                            item_enum.ident
                        ),
                    ),
                    item_enum.span(),
                );
            }
            Item::Union(item_union) if is_public(&item_union.vis) => {
                let _ = push_fact(
                    facts,
                    source,
                    qualify(module, &item_union.ident.to_string()),
                    "rust_public_type",
                    "public",
                    signature_with_cfg(
                        inherited_cfg,
                        &item_union.attrs,
                        format!(
                            "{} union {}",
                            item_union.vis.to_token_stream(),
                            item_union.ident
                        ),
                    ),
                    item_union.span(),
                );
            }
            Item::Type(item_type) if is_public(&item_type.vis) => {
                let _ = push_fact(
                    facts,
                    source,
                    qualify(module, &item_type.ident.to_string()),
                    "rust_public_type",
                    "public",
                    signature_with_cfg(
                        inherited_cfg,
                        &item_type.attrs,
                        format!(
                            "{} type {}",
                            item_type.vis.to_token_stream(),
                            item_type.ident
                        ),
                    ),
                    item_type.span(),
                );
            }
            Item::Const(item_const) if is_public(&item_const.vis) => {
                let _ = push_fact(
                    facts,
                    source,
                    qualify(module, &item_const.ident.to_string()),
                    "rust_public_constant",
                    "public",
                    signature_with_cfg(
                        inherited_cfg,
                        &item_const.attrs,
                        format!(
                            "{} const {}",
                            item_const.vis.to_token_stream(),
                            item_const.ident
                        ),
                    ),
                    item_const.span(),
                );
            }
            Item::Static(item_static) if is_public(&item_static.vis) => {
                let _ = push_fact(
                    facts,
                    source,
                    qualify(module, &item_static.ident.to_string()),
                    "rust_public_constant",
                    "public",
                    signature_with_cfg(
                        inherited_cfg,
                        &item_static.attrs,
                        format!(
                            "{} static {}",
                            item_static.vis.to_token_stream(),
                            item_static.ident
                        ),
                    ),
                    item_static.span(),
                );
            }
            Item::Macro(item_macro)
                if item_macro
                    .mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "fuzz_target") =>
            {
                fuzz_target_ordinal += 1;
                let _ = push_fact(
                    facts,
                    source,
                    format!("{module}::fuzz_target[{fuzz_target_ordinal}]"),
                    "rust_fuzz_target",
                    "private",
                    signature_with_cfg(
                        inherited_cfg,
                        &item_macro.attrs,
                        "fuzz_target!(closure)".to_string(),
                    ),
                    item_macro.span(),
                );
            }
            Item::Mod(item_mod) => {
                if let Some((_, nested)) = &item_mod.content {
                    let module_cfg = merged_cfg_predicates(inherited_cfg, &item_mod.attrs);
                    scan_items(
                        &qualify(module, &item_mod.ident.to_string()),
                        nested,
                        source,
                        &module_cfg,
                        facts,
                    );
                }
            }
            _ => {}
        }
    }
}

fn push_fact(
    facts: &mut Vec<RustFact>,
    source: &str,
    qualified_symbol: String,
    item_kind: &str,
    visibility: &str,
    signature: String,
    span: Span,
) -> Option<usize> {
    if let Some((body_start, body_end, line)) = span_range(source, span) {
        let index = facts.len();
        facts.push(RustFact {
            qualified_symbol,
            item_kind: item_kind.to_string(),
            visibility: visibility.to_string(),
            signature: normalize_signature(&signature),
            body_start,
            body_end,
            line,
            risk_signals: Vec::new(),
        });
        Some(index)
    } else {
        None
    }
}

fn scan_expressions(
    parent: &str,
    block: &syn::Block,
    source: &str,
    inherited_cfg: &[String],
    attributes: &[Attribute],
    facts: &mut Vec<RustFact>,
) -> Vec<String> {
    let cfg_predicates = merged_cfg_predicates(inherited_cfg, attributes);
    let mut visitor = ExpressionFacts {
        parent,
        source,
        facts,
        cfg_predicates,
        registration_ordinal: 0,
        unsafe_ordinal: 0,
        risk_signals: BTreeSet::new(),
    };
    visitor.visit_block(block);
    visitor.risk_signals.into_iter().collect()
}

struct ExpressionFacts<'a> {
    parent: &'a str,
    source: &'a str,
    facts: &'a mut Vec<RustFact>,
    cfg_predicates: Vec<String>,
    registration_ordinal: usize,
    unsafe_ordinal: usize,
    risk_signals: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ExpressionFacts<'_> {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        add_risks_for_name(&compact_tokens(&node.func), &mut self.risk_signals);
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        add_risks_for_name(&node.method.to_string(), &mut self.risk_signals);
        if node.method == "ok" {
            self.risk_signals
                .insert("broad_error_suppression".to_string());
        }
        if node.method == "register" {
            self.registration_ordinal += 1;
            let key = node
                .args
                .first()
                .and_then(|argument| match argument {
                    syn::Expr::Lit(literal) => match &literal.lit {
                        syn::Lit::Str(value) => Some(value.value()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or_else(|| "dynamic".to_string());
            let index = push_fact(
                self.facts,
                self.source,
                format!(
                    "{}::register[{}:{}]",
                    self.parent, self.registration_ordinal, key
                ),
                "rust_registration",
                "private",
                signature_with_predicates(&self.cfg_predicates, node.to_token_stream().to_string()),
                node.span(),
            );
            set_fact_risks(self.facts, index, vec!["state_mutation".to_string()]);
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast ExprAssign) {
        self.risk_signals.insert("state_mutation".to_string());
        visit::visit_expr_assign(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast ExprMacro) {
        add_risks_for_name(&compact_tokens(&node.mac.path), &mut self.risk_signals);
        visit::visit_expr_macro(self, node);
    }

    fn visit_expr_unsafe(&mut self, node: &'ast ExprUnsafe) {
        self.unsafe_ordinal += 1;
        let index = push_fact(
            self.facts,
            self.source,
            format!("{}::unsafe[{}]", self.parent, self.unsafe_ordinal),
            "rust_unsafe_site",
            "private",
            signature_with_predicates(&self.cfg_predicates, "unsafe".to_string()),
            node.span(),
        );
        set_fact_risks(self.facts, index, vec!["unsafe_or_ffi".to_string()]);
        self.risk_signals.insert("unsafe_or_ffi".to_string());
        visit::visit_expr_unsafe(self, node);
    }
}

fn set_fact_risks(facts: &mut [RustFact], index: Option<usize>, risks: Vec<String>) {
    if let Some(fact) = index.and_then(|index| facts.get_mut(index)) {
        fact.risk_signals = risks;
        fact.risk_signals.sort();
        fact.risk_signals.dedup();
    }
}

fn add_risks_for_name(name: &str, signals: &mut BTreeSet<String>) {
    let normalized = name.to_ascii_lowercase();
    let policies: [(&str, &[&str]); 12] = [
        (
            "host_time",
            &["systemtime::now", "utc::now", "local::now", "current_dt"],
        ),
        (
            "randomness",
            &["random", "thread_rng", "osrng", "getrandom"],
        ),
        (
            "filesystem_or_environment",
            &[
                "std::fs",
                "tokio::fs",
                "std::env",
                "env::var",
                "read_to_string",
                "write_all",
            ],
        ),
        (
            "network_io",
            &[
                "tcpstream",
                "tcplistener",
                "udpsocket",
                "reqwest",
                "ureq",
                "websocket",
                "zenoh",
            ],
        ),
        (
            "subprocess_or_thread",
            &[
                "command::new",
                "process::command",
                "thread::spawn",
                "tokio::spawn",
                "spawn_blocking",
            ],
        ),
        (
            "state_mutation",
            &[
                "insert",
                "push",
                "remove",
                "clear",
                "store",
                "set_state",
                "update",
            ],
        ),
        (
            "output_or_safe_state_commit",
            &[
                "commit_output",
                "safe_state",
                "force_output",
                "release_output",
                "println",
                "eprintln",
            ],
        ),
        (
            "authorization",
            &["authorize", "authorization", "authz", "permission", "role"],
        ),
        (
            "fault_mapping",
            &["map_err", "fault", "error_response", "panic"],
        ),
        (
            "protocol_response_or_status",
            &["response", "status", "health", "protocol_result"],
        ),
        ("broad_error_suppression", &["unwrap_or_default"]),
        (
            "persistence_retain_reload_restart",
            &["persist", "retain", "reload", "restart"],
        ),
    ];
    for (signal, needles) in policies {
        if needles
            .iter()
            .any(|needle| risk_name_matches(&normalized, needle))
        {
            signals.insert(signal.to_string());
        }
    }
}

fn risk_name_matches(normalized: &str, needle: &str) -> bool {
    if needle.contains("::") || needle.contains('.') {
        return normalized.contains(needle);
    }
    normalized
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|identifier| !identifier.is_empty())
        .any(|identifier| {
            identifier == needle
                || (!needle.contains('_') && identifier.split('_').any(|token| token == needle))
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

fn visibility(visibility: &Visibility) -> &'static str {
    if is_public(visibility) {
        "public"
    } else {
        "private"
    }
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_string()
    } else {
        format!("{module}::{name}")
    }
}

fn compact_tokens<T: ToTokens + ?Sized>(tokens: &T) -> String {
    tokens.to_token_stream().to_string().replace(' ', "")
}

fn qualified_self_type(module: &str, self_type: &Type) -> String {
    let compact = compact_tokens(self_type);
    match self_type {
        Type::Path(path) if path.qself.is_none() && path.path.leading_colon.is_none() => {
            let first = path
                .path
                .segments
                .first()
                .map(|segment| segment.ident.to_string());
            if matches!(first.as_deref(), Some("crate" | "self" | "super")) {
                compact
            } else {
                qualify(module, &compact)
            }
        }
        _ => compact,
    }
}

fn normalize_signature(signature: &str) -> String {
    signature.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn signature_with_cfg(inherited: &[String], attributes: &[Attribute], signature: String) -> String {
    signature_with_predicates(&merged_cfg_predicates(inherited, attributes), signature)
}

fn merged_cfg_predicates(inherited: &[String], attributes: &[Attribute]) -> Vec<String> {
    let mut predicates = inherited.to_vec();
    predicates.extend(
        attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("cfg"))
            .map(|attribute| compact_tokens(&attribute.meta)),
    );
    predicates.sort();
    predicates.dedup();
    predicates
}

fn signature_with_predicates(predicates: &[String], signature: String) -> String {
    if predicates.is_empty() {
        signature
    } else {
        format!(
            "{} {signature}",
            predicates
                .iter()
                .map(|predicate| format!("#[{predicate}]"))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn span_range(source: &str, span: Span) -> Option<(usize, usize, usize)> {
    let start = span.start();
    let end = span.end();
    let body_start = line_column_to_offset(source, start.line, start.column)?;
    let body_end = line_column_to_offset(source, end.line, end.column)?;
    (body_end >= body_start).then_some((body_start, body_end, start.line))
}

fn line_column_to_offset(source: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 {
        return None;
    }
    let mut offset = 0;
    for (index, segment) in source.split_inclusive('\n').enumerate() {
        if index + 1 == line {
            return (column <= segment.len()).then_some(offset + column);
        }
        offset += segment.len();
    }
    if line == source.lines().count() + 1 && column == 0 {
        Some(source.len())
    } else {
        None
    }
}
