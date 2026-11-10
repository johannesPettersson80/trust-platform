//! What a program reaches through its calls (specification 28 §6; control-studio-runtime-protocol annex E2.3): the globals that
//! the POUs it calls read and write are its own accesses, so a function block that writes a global writes it in each task whose
//! programs call it, directly or through further calls, as CODESYS Static Analysis follows the call tree. A call's callee is the
//! POU the type checker resolved it to: a function, the function block of an instance's call or a method. An action's statements
//! are part of the POU that declares it, so calling an action reaches that POU. A call through an interface names no body, so the
//! methods it may run are reached by no program, and their writes count for no single task. A POU is keyed by its file and its
//! name's range, which every file's symbol table gives alike.

use super::*;

/// A POU, by its file and the range of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PouKey {
    file: FileId,
    name: TextRange,
}

/// The key of the POU `pou` of `file`, when it has a name.
pub(super) fn pou_key(file: FileId, pou: &SyntaxNode) -> Option<PouKey> {
    name_from_node(pou).map(|(_, name)| PouKey { file, name })
}

/// What one file's type check resolved that the call graph reads: the file's symbol table, and the symbol each call node
/// resolved to, by the node's range (`FileAnalysis::resolved_symbols`).
pub(in crate::db) struct CallFacts {
    pub(in crate::db) symbols: Arc<SymbolTable>,
    pub(in crate::db) resolved: Arc<FxHashMap<(u32, u32), SymbolId>>,
}

/// Each POU's own accesses to the globals, and the POUs its calls run.
pub(super) struct CallGraph {
    accesses: FxHashMap<PouKey, ProgramAccess>,
    calls: FxHashMap<PouKey, FxHashSet<PouKey>>,
}

impl CallGraph {
    /// The graph of every POU of `roots`: its accesses to the globals of `globals_by_key`, resolved in `symbols`, and its calls,
    /// resolved by its file's type check (`call_facts`).
    pub(super) fn build(
        symbols: &SymbolTable,
        roots: &[(FileId, SyntaxNode)],
        globals_by_key: &FxHashMap<GlobalKey, GlobalCandidate>,
        written_arguments: &dyn Fn(FileId) -> Arc<FxHashSet<(u32, u32)>>,
        call_facts: &dyn Fn(FileId) -> Option<CallFacts>,
    ) -> Self {
        let mut graph = Self {
            accesses: FxHashMap::default(),
            calls: FxHashMap::default(),
        };
        for (file_id, root) in roots {
            let written = written_arguments(*file_id);
            let facts = call_facts(*file_id);
            for pou in root.descendants().filter(|node| is_pou_kind(node.kind())) {
                let Some(key) = pou_key(*file_id, &pou) else {
                    continue;
                };
                let mut access = ProgramAccess {
                    reads: FxHashSet::default(),
                    writes: FxHashSet::default(),
                };
                collect_pou_accesses(symbols, globals_by_key, &pou, &written, &mut access);
                graph.accesses.insert(key, access);
                let callees: FxHashSet<PouKey> = facts
                    .iter()
                    .flat_map(|facts| {
                        pou.descendants()
                            .filter(|node| {
                                node.kind() == SyntaxKind::CallExpr && belongs_to_pou(node, &pou)
                            })
                            .filter_map(|call| callee(*file_id, facts, &call))
                            .collect::<Vec<_>>()
                    })
                    .collect();
                graph.calls.insert(key, callees);
            }
        }
        graph
    }

    /// The POUs that `start` reaches through its calls, itself included.
    pub(super) fn reached(&self, start: PouKey) -> FxHashSet<PouKey> {
        let mut reached = FxHashSet::default();
        let mut pending = vec![start];
        while let Some(key) = pending.pop() {
            if reached.insert(key) {
                pending.extend(self.calls.get(&key).into_iter().flatten().copied());
            }
        }
        reached
    }

    /// The accesses of the POUs `reached`, together: a program's, with those of every POU it reaches.
    pub(super) fn access_of(&self, reached: &FxHashSet<PouKey>) -> ProgramAccess {
        let mut access = ProgramAccess {
            reads: FxHashSet::default(),
            writes: FxHashSet::default(),
        };
        for own in reached.iter().filter_map(|key| self.accesses.get(key)) {
            access.reads.extend(own.reads.iter().copied());
            access.writes.extend(own.writes.iter().copied());
        }
        access
    }

    /// The globals that a POU outside `reached` writes: one that no program reaches through a resolved call, such as a method an
    /// interface's call may run, which any task may run.
    pub(super) fn writes_outside(&self, reached: &FxHashSet<PouKey>) -> FxHashSet<SymbolId> {
        self.accesses
            .iter()
            .filter(|(key, _)| !reached.contains(key))
            .flat_map(|(_, access)| access.writes.iter().copied())
            .collect()
    }
}

/// The POU that the call node `call` of `file` runs, as the file's type check resolved it; for an action, the POU that declares
/// it, which holds the action's statements.
fn callee(file: FileId, facts: &CallFacts, call: &SyntaxNode) -> Option<PouKey> {
    let range = call.text_range();
    let id = facts
        .resolved
        .get(&(u32::from(range.start()), u32::from(range.end())))?;
    let mut symbol = facts.symbols.get(*id)?;
    if matches!(symbol.kind, SymbolKind::Action) {
        symbol = facts.symbols.get(symbol.parent?)?;
    }
    Some(PouKey {
        file: symbol.origin.map_or(file, |origin| origin.file_id),
        name: symbol.range,
    })
}
