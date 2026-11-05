use rustc_hash::{FxHashMap, FxHashSet};

use super::super::queries::*;
use super::super::*;
use super::context::find_symbol_by_name_range;

/// Canonical identity of a POU: the file that declares it and the symbol id in
/// that file's own table. An imported symbol maps back through its origin, so
/// one POU has one key project-wide.
pub(in crate::db) type PouKey = (FileId, SymbolId);

/// The project's clock-source index: every POU whose body reads the host wall
/// clock (`CURRENT_DT`), directly or through the POUs it calls (specification
/// 07, "Runtime Clock Sources"; specification 28 section 5).
#[derive(Debug, Default, PartialEq, Eq)]
pub(in crate::db) struct HostClockSources {
    readers: FxHashSet<PouKey>,
}

impl HostClockSources {
    pub(in crate::db) fn reads_host_clock(&self, pou: &PouKey) -> bool {
        self.readers.contains(pou)
    }
}

pub(in crate::db) fn check_nondeterminism(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    file_id: FileId,
    resolved_callees: &FxHashMap<(u32, u32), SymbolId>,
    clock_sources: &HostClockSources,
    diagnostics: &mut DiagnosticBuilder,
) {
    for symbol in symbols.iter() {
        if symbol.origin.is_some() || symbol.range.is_empty() {
            continue;
        }

        if let Some(address) = symbol.direct_address.as_deref()
            && is_io_address(address)
            && in_reusable_code(symbols, symbol)
        {
            diagnostics.warning(
                DiagnosticCode::NondeterministicIo,
                symbol.range,
                format!(
                    "direct I/O address '{}' on '{}' may introduce nondeterministic timing",
                    address, symbol.name
                ),
            );
        }
    }

    for call in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::CallExpr)
    {
        let Some(callee) = call.children().next() else {
            continue;
        };
        let call_range = call.text_range();
        let call_key = (u32::from(call_range.start()), u32::from(call_range.end()));

        // A call of the product function itself (specification 07). A user
        // declaration of the same name shadows it and is an ordinary call.
        if callee.kind() == SyntaxKind::NameRef {
            let name_text = callee.text().to_string();
            let name = name_text.trim();
            if name.eq_ignore_ascii_case("CURRENT_DT") && symbols.lookup(name).is_none() {
                diagnostics.warning(
                    DiagnosticCode::NondeterministicTimeDate,
                    call_target_range(&callee),
                    "CURRENT_DT reads the host clock, which replay does not control".to_string(),
                );
            }
        }

        // A call of a POU whose body reaches the host clock, directly or
        // through the POUs it calls. The type checker records the callable
        // every call binds (specification 44 section 3); a call that resolves
        // to nothing readable warns no more than an unread clock.
        let Some(&callee_id) = resolved_callees.get(&call_key) else {
            continue;
        };
        let canonical = canonical_pou(symbols, file_id, callee_id);
        if !clock_sources.reads_host_clock(&canonical) {
            continue;
        }
        let callee_name = symbols
            .get(callee_id)
            .map(|symbol| symbol.name.clone())
            .unwrap_or_default();
        let range = if matches!(callee.kind(), SyntaxKind::NameRef | SyntaxKind::FieldExpr) {
            call_target_range(&callee)
        } else {
            call_range
        };
        diagnostics.warning(
            DiagnosticCode::NondeterministicTimeDate,
            range,
            format!(
                "'{callee_name}' reads the host clock through CURRENT_DT, which replay does not control"
            ),
        );
    }
}

fn canonical_pou(symbols: &SymbolTable, file_id: FileId, symbol_id: SymbolId) -> PouKey {
    match symbols.get(symbol_id).and_then(|symbol| symbol.origin) {
        Some(origin) => (origin.file_id, origin.symbol_id),
        None => (file_id, symbol_id),
    }
}

/// The range a call-site warning points at: the called name's identifier, not
/// the node extent that carries leading trivia.
fn call_target_range(callee: &SyntaxNode) -> TextRange {
    let name_node = if callee.kind() == SyntaxKind::FieldExpr {
        callee
            .children()
            .filter(|child| child.kind() == SyntaxKind::NameRef)
            .last()
    } else {
        Some(callee.clone())
    };
    name_node
        .and_then(|node| first_ident_token(&node))
        .map(|token| token.text_range())
        .unwrap_or_else(|| callee.text_range())
}

/// Builds the project-wide index. `files` holds every project file with its
/// own symbol table and parsed root.
pub(in crate::db) fn compute_host_clock_sources(
    files: &[(FileId, &SymbolTable, SyntaxNode)],
) -> HostClockSources {
    let mut direct: FxHashSet<PouKey> = FxHashSet::default();
    let mut call_targets: FxHashMap<PouKey, Vec<PouKey>> = FxHashMap::default();
    // Callable POUs by upper-cased name: the bare-name call targets, and the
    // member-name targets of method calls.
    let mut callables_by_name: FxHashMap<SmolStr, Vec<PouKey>> = FxHashMap::default();
    let mut methods_by_name: FxHashMap<SmolStr, Vec<PouKey>> = FxHashMap::default();

    // Pass one: every POU that its file's table can anchor.
    let mut pous = Vec::new();
    for (file_id, symbols, root) in files {
        for pou in pou_nodes(root) {
            let Some((name, name_range)) = name_from_node(&pou) else {
                continue;
            };
            let Some(pou_id) = find_symbol_by_name_range(symbols, name.as_str(), name_range) else {
                continue;
            };
            let key: PouKey = (*file_id, pou_id);
            register_callable(
                symbols,
                pou_id,
                key,
                &mut callables_by_name,
                &mut methods_by_name,
            );
            pous.push((key, pou));
        }
    }

    // Pass two: the bodies, now that every call target is registered.
    for (key, pou) in &pous {
        let mut reads = false;
        let mut targets = Vec::new();
        collect_body_calls(pou, &mut |call| match call {
            BodyCall::CurrentDt => reads = true,
            BodyCall::Named(name) => {
                if let Some(found) = callables_by_name.get(&name) {
                    targets.extend(found.iter().copied());
                }
                if let Some(found) = methods_by_name.get(&name) {
                    targets.extend(found.iter().copied());
                }
            }
        });
        if reads {
            direct.insert(*key);
        }
        if !targets.is_empty() {
            call_targets.insert(*key, targets);
        }
    }

    // Reading the clock propagates from callee to caller until it is stable.
    let mut readers = direct;
    let mut changed = true;
    while changed {
        changed = false;
        for (caller, targets) in &call_targets {
            if !readers.contains(caller) && targets.iter().any(|target| readers.contains(target)) {
                readers.insert(*caller);
                changed = true;
            }
        }
    }
    HostClockSources { readers }
}

enum BodyCall {
    CurrentDt,
    Named(SmolStr),
}

fn pou_nodes(root: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> + '_ {
    root.descendants().filter(|node| {
        matches!(
            node.kind(),
            SyntaxKind::Program
                | SyntaxKind::Function
                | SyntaxKind::FunctionBlock
                | SyntaxKind::Method
                | SyntaxKind::Property
                | SyntaxKind::Action
        )
    })
}

fn register_callable(
    symbols: &SymbolTable,
    pou_id: SymbolId,
    key: PouKey,
    callables_by_name: &mut FxHashMap<SmolStr, Vec<PouKey>>,
    methods_by_name: &mut FxHashMap<SmolStr, Vec<PouKey>>,
) {
    let Some(symbol) = symbols.get(pou_id) else {
        return;
    };
    let name = SmolStr::new(symbol.name.to_ascii_uppercase());
    match symbol.kind {
        SymbolKind::Function { .. }
        | SymbolKind::FunctionBlock
        | SymbolKind::Program
        | SymbolKind::Action => {
            callables_by_name.entry(name).or_default().push(key);
        }
        SymbolKind::Method { .. } => {
            methods_by_name.entry(name).or_default().push(key);
        }
        _ => {}
    }
}

fn collect_body_calls(pou: &SyntaxNode, out: &mut impl FnMut(BodyCall)) {
    let mut stack: Vec<SyntaxNode> = pou.children().collect();
    while let Some(node) = stack.pop() {
        // A nested POU or action is its own unit in the graph.
        if node.kind().is_pou_declaration() || node.kind() == SyntaxKind::Action {
            continue;
        }
        if node.kind() == SyntaxKind::CallExpr
            && let Some(callee) = node.children().next()
        {
            match callee.kind() {
                SyntaxKind::NameRef => {
                    let name_text = callee.text().to_string();
                    let name = name_text.trim().to_ascii_uppercase();
                    if name == "CURRENT_DT" {
                        out(BodyCall::CurrentDt);
                    } else {
                        out(BodyCall::Named(SmolStr::new(name)));
                    }
                }
                SyntaxKind::FieldExpr => {
                    if let Some(member) = callee
                        .children()
                        .filter(|child| child.kind() == SyntaxKind::NameRef)
                        .last()
                    {
                        let member_text = member.text().to_string();
                        out(BodyCall::Named(SmolStr::new(
                            member_text.trim().to_ascii_uppercase(),
                        )));
                    }
                }
                _ => {}
            }
        }
        stack.extend(node.children());
    }
}

/// Whether a declaration lies in reusable code: a function block, function, method or class,
/// where a direct I/O address ties every use to one channel and to the I/O image's timing. A
/// program's or configuration's I/O list is the normal place for addresses.
fn in_reusable_code(symbols: &SymbolTable, symbol: &Symbol) -> bool {
    let mut parent = symbol.parent;
    for _ in 0..16 {
        let Some(owner) = parent.and_then(|id| symbols.get(id)) else {
            return false;
        };
        match owner.kind {
            SymbolKind::FunctionBlock
            | SymbolKind::Function { .. }
            | SymbolKind::Method { .. }
            | SymbolKind::Class => return true,
            SymbolKind::Program
            | SymbolKind::Configuration
            | SymbolKind::Resource
            | SymbolKind::Namespace => return false,
            _ => parent = owner.parent,
        }
    }
    false
}

fn is_io_address(address: &str) -> bool {
    let trimmed = address.trim();
    let mut chars = trimmed.chars();
    if let Some('%') = chars.next()
        && let Some(prefix) = chars.next()
    {
        return matches!(prefix.to_ascii_uppercase(), 'I' | 'Q');
    }
    false
}

#[cfg(test)]
#[path = "nondeterminism/contract_tests.rs"]
mod contract_tests;
