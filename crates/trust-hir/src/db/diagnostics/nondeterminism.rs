use rustc_hash::FxHashMap;

use super::super::queries::*;
use super::super::*;
use super::attributes::declares_attribute;
use super::context::find_symbol_by_name_range;
use crate::types::Type;

/// Canonical identity of a POU: the file that declares it and the symbol id in
/// that file's own table. An imported symbol maps back through its origin, so
/// one POU has one key project-wide.
pub(in crate::db) type PouKey = (FileId, SymbolId);

/// The clock source a reader reaches: the product host clock, or a POU that
/// declares `reads_host_clock` (specification 07, "Declared Clock Sources";
/// specification 28 section 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::db) enum ClockSource {
    /// The product host clock: a body that calls `CURRENT_DT`.
    CurrentDt,
    /// A POU carrying the `{attribute 'reads_host_clock'}` pragma: its key,
    /// which tells the declared POU itself from a same-named reader, and the
    /// POU's name for the message.
    Declared { key: PouKey, name: SmolStr },
}

/// A clock source with the document rank of the direct reader that carries
/// it. A call that reaches more than one source names the one with the
/// smallest rank, so the named source never depends on iteration order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClockEntry {
    rank: u32,
    source: ClockSource,
}

/// The project's clock-source index: every POU whose body reads the host wall
/// clock (`CURRENT_DT`), directly or through the POUs it calls, plus every
/// POU that declares the read (specification 07; specification 28 section 5).
#[derive(Debug, Default, PartialEq, Eq)]
pub(in crate::db) struct HostClockSources {
    sources: FxHashMap<PouKey, ClockEntry>,
}

impl HostClockSources {
    /// The source a reader reaches, named by the call-site message.
    pub(in crate::db) fn source_reaching(&self, pou: &PouKey) -> Option<&ClockSource> {
        self.sources.get(pou).map(|entry| &entry.source)
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
        // through the POUs it calls, or of a POU that declares the read
        // (specification 07, "Declared Clock Sources"). The type checker
        // records the callable every call binds (specification 44 section 3);
        // a call that resolves to nothing readable warns no more than an
        // unread clock.
        let Some(&callee_id) = resolved_callees.get(&call_key) else {
            continue;
        };
        let canonical = canonical_pou(symbols, file_id, callee_id);
        let Some(source) = clock_sources.source_reaching(&canonical) else {
            continue;
        };
        let callee_name = symbols
            .get(callee_id)
            .map(|symbol| symbol.name.clone())
            .unwrap_or_default();
        let range = if matches!(callee.kind(), SyntaxKind::NameRef | SyntaxKind::FieldExpr) {
            call_target_range(&callee)
        } else {
            call_range
        };
        let message = match source {
            ClockSource::CurrentDt => format!(
                "'{callee_name}' reads the host clock through CURRENT_DT, which replay does not control"
            ),
            ClockSource::Declared { key: declared, .. } if *declared == canonical => format!(
                "'{callee_name}' reads the host clock (declared reads_host_clock), which replay does not control"
            ),
            ClockSource::Declared { name: source_name, .. } => format!(
                "'{callee_name}' reads the host clock through '{source_name}' (declared reads_host_clock), which replay does not control"
            ),
        };
        diagnostics.warning(DiagnosticCode::NondeterministicTimeDate, range, message);
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
/// merged symbol table and parsed root. Each body call resolves in the scope
/// of the POU whose body contains it, so an instance's call maps to the block
/// the variable declares, `inst.M()` maps to that type's method, and a bare
/// call never takes a same-named method's clock (specification 28 section 5).
/// A POU whose leading pragma trivia carries `reads_host_clock` seeds as a
/// reader without a body read (specification 07, "Declared Clock Sources").
pub(in crate::db) fn compute_host_clock_sources(
    files: &[(FileId, &SymbolTable, SyntaxNode)],
) -> HostClockSources {
    let mut direct: FxHashMap<PouKey, ClockEntry> = FxHashMap::default();
    let mut call_targets: FxHashMap<PouKey, Vec<PouKey>> = FxHashMap::default();
    let mut rank: u32 = 0;

    for (file_id, symbols, root) in files {
        for pou in pou_nodes(root) {
            let Some((name, name_range)) = name_from_node(&pou) else {
                continue;
            };
            let Some(pou_id) = find_symbol_by_name_range(symbols, name.as_str(), name_range) else {
                continue;
            };
            let key: PouKey = (*file_id, pou_id);
            let entry_rank = rank;
            rank += 1;
            let scope_id = symbols.scope_for_owner(pou_id).unwrap_or(ScopeId::GLOBAL);
            let mut reads = false;
            let mut targets: Vec<PouKey> = Vec::new();
            collect_body_calls(
                symbols,
                *file_id,
                pou_id,
                scope_id,
                &pou,
                &mut |call| match call {
                    BodyCall::CurrentDt => reads = true,
                    BodyCall::Named(target) => targets.push(target),
                },
            );
            if reads {
                // The body's own read is the concrete source; a declaration
                // on the same POU adds nothing (specification 07: one
                // source, not two).
                direct.insert(
                    key,
                    ClockEntry {
                        rank: entry_rank,
                        source: ClockSource::CurrentDt,
                    },
                );
            } else if declares_attribute(&pou, "reads_host_clock") {
                direct.insert(
                    key,
                    ClockEntry {
                        rank: entry_rank,
                        source: ClockSource::Declared { key, name },
                    },
                );
            }
            if !targets.is_empty() {
                call_targets.insert(key, targets);
            }
        }
    }

    // Reaching a source propagates from callee to caller until it is stable.
    // The smallest rank wins at every step, so a call reaching more than one
    // source names the first one in the project's file order and document
    // order, whatever order the map iterates in.
    let mut sources = direct;
    let mut changed = true;
    while changed {
        changed = false;
        for (caller, targets) in &call_targets {
            let best = targets
                .iter()
                .filter_map(|target| sources.get(target))
                .min_by_key(|entry| entry.rank)
                .cloned();
            let Some(best) = best else {
                continue;
            };
            match sources.get(caller) {
                Some(entry) if entry.rank <= best.rank => {}
                _ => {
                    sources.insert(*caller, best);
                    changed = true;
                }
            }
        }
    }
    HostClockSources { sources }
}

enum BodyCall {
    CurrentDt,
    Named(PouKey),
}

/// What a body's bare callee name binds to.
enum BareCall {
    /// The product host clock: the name is `CURRENT_DT` and no declaration
    /// shadows it.
    HostClock,
    /// A resolved callable POU in the project.
    Target(PouKey),
    /// Nothing resolvable: the call stays silent, like every call that cannot
    /// be resolved (specification 28 section 5).
    Unresolved,
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

/// Scoped name resolution for a body call: the calling scope's chain first,
/// then the table's global name. The type checker resolves the same way.
fn resolve_in_scope(symbols: &SymbolTable, name: &str, scope_id: ScopeId) -> Option<SymbolId> {
    symbols
        .resolve(name, scope_id)
        .or_else(|| symbols.lookup(name))
}

/// What a bare callee name binds to, resolved in the calling scope.
fn bare_call_target(
    symbols: &SymbolTable,
    file_id: FileId,
    name: &str,
    scope_id: ScopeId,
    pou_id: SymbolId,
) -> BareCall {
    if let Some(id) = resolve_in_scope(symbols, name, scope_id) {
        let Some(symbol) = symbols.get(id) else {
            return BareCall::Unresolved;
        };
        match &symbol.kind {
            SymbolKind::Function { .. }
            | SymbolKind::FunctionBlock
            | SymbolKind::Program
            | SymbolKind::Action => {
                return BareCall::Target(canonical_pou(symbols, file_id, id));
            }
            SymbolKind::Variable { .. } => {
                return match block_instance_target(symbols, file_id, symbol) {
                    Some(target) => BareCall::Target(target),
                    None => BareCall::Unresolved,
                };
            }
            // A method is never reachable by bare name alone; the
            // implicit-self form resolves below.
            _ => return BareCall::Unresolved,
        }
    }
    // The name resolved to nothing, so a call of `CURRENT_DT` is the product
    // host clock; any other unresolved name can only be the implicit-self
    // form of the enclosing block (vendor dialect).
    if name.eq_ignore_ascii_case("CURRENT_DT") {
        return BareCall::HostClock;
    }
    match implicit_self_target(symbols, file_id, pou_id, name) {
        Some(target) => BareCall::Target(target),
        None => BareCall::Unresolved,
    }
}

/// The block an instance's call runs: the function block that the instance
/// variable declares.
fn block_instance_target(
    symbols: &SymbolTable,
    file_id: FileId,
    symbol: &Symbol,
) -> Option<PouKey> {
    let base = symbols.resolve_alias_type(symbol.type_id);
    let Type::FunctionBlock { name } = symbols.type_by_id(base)? else {
        return None;
    };
    let block_id = symbols.resolve_global_or_qualified_name(name.as_str())?;
    Some(canonical_pou(symbols, file_id, block_id))
}

/// `inst.M()`: the method or action of the type the instance declares, a
/// program's child, or a namespace-qualified function — in the type checker's
/// own order. A base that resolves to nothing usable stays silent, like every
/// call that cannot be resolved.
fn member_call_target(
    symbols: &SymbolTable,
    file_id: FileId,
    callee: &SyntaxNode,
    scope_id: ScopeId,
) -> Option<PouKey> {
    // The dotted name's parts, as written: a FieldExpr holds them as name
    // nodes (`NameRef` for the head, `Name` for the members), each carrying
    // its own leading trivia.
    let parts: Vec<SmolStr> = callee
        .children()
        .filter(|child| matches!(child.kind(), SyntaxKind::NameRef | SyntaxKind::Name))
        .map(|child| {
            let text = child.text().to_string();
            SmolStr::new(text.trim())
        })
        .collect();
    if parts.len() < 2 {
        return None;
    }
    let member_name = parts.last()?.as_str();

    if parts.len() == 2
        && let Some(base_id) = resolve_in_scope(symbols, parts[0].as_str(), scope_id)
    {
        let base_symbol = symbols.get(base_id)?;
        let member_id = match &base_symbol.kind {
            SymbolKind::Variable { .. } => {
                symbols.resolve_member_symbol_in_type(base_symbol.type_id, member_name)?
            }
            SymbolKind::Program => symbols.resolve_child_symbol(base_id, member_name)?,
            SymbolKind::Namespace => {
                return namespace_call_target(symbols, file_id, &parts, scope_id);
            }
            _ => return None,
        };
        return match symbols.get(member_id).map(|symbol| &symbol.kind) {
            Some(SymbolKind::Function { .. } | SymbolKind::Method { .. } | SymbolKind::Action) => {
                Some(canonical_pou(symbols, file_id, member_id))
            }
            _ => None,
        };
    }
    // Namespace-qualified (`WagoAppTime.FuGetDateAndTime()`): the same
    // resolution the type checker applies; a local named like the namespace
    // hides it (specification 09 1.3) and the call stays silent.
    namespace_call_target(symbols, file_id, &parts, scope_id)
}

fn namespace_call_target(
    symbols: &SymbolTable,
    file_id: FileId,
    parts: &[SmolStr],
    scope_id: ScopeId,
) -> Option<PouKey> {
    if symbols.hides_global_head(parts[0].as_str(), scope_id) {
        return None;
    }
    let member_id = symbols.resolve_qualified(parts)?;
    match symbols.get(member_id).map(|symbol| &symbol.kind) {
        Some(SymbolKind::Function { .. } | SymbolKind::Method { .. } | SymbolKind::Action) => {
            Some(canonical_pou(symbols, file_id, member_id))
        }
        _ => None,
    }
}

/// The implicit-self form (vendor dialect): a bare name that resolves nowhere
/// can still be a method or an action of the enclosing block or its owner.
fn implicit_self_target(
    symbols: &SymbolTable,
    file_id: FileId,
    pou_id: SymbolId,
    name: &str,
) -> Option<PouKey> {
    let mut owner = Some(pou_id);
    for _ in 0..4 {
        let current = owner?;
        if let Some(id) = symbols.resolve_child_symbol(current, name)
            && matches!(
                symbols.get(id).map(|symbol| &symbol.kind),
                Some(SymbolKind::Method { .. } | SymbolKind::Action)
            )
        {
            return Some(canonical_pou(symbols, file_id, id));
        }
        owner = symbols.get(current).and_then(|symbol| symbol.parent);
    }
    None
}

fn collect_body_calls(
    symbols: &SymbolTable,
    file_id: FileId,
    pou_id: SymbolId,
    scope_id: ScopeId,
    pou: &SyntaxNode,
    out: &mut impl FnMut(BodyCall),
) {
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
                    match bare_call_target(symbols, file_id, name_text.trim(), scope_id, pou_id) {
                        BareCall::HostClock => out(BodyCall::CurrentDt),
                        BareCall::Target(target) => out(BodyCall::Named(target)),
                        BareCall::Unresolved => {}
                    }
                }
                SyntaxKind::FieldExpr => {
                    if let Some(target) = member_call_target(symbols, file_id, &callee, scope_id) {
                        out(BodyCall::Named(target));
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
