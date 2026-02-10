# Salsa Integration Audit Report

> Historical note (2026-02-10): this report captures pre-remediation findings.
> The tracked issues were addressed on `spike/salsa-stage1-gate` during the Salsa
> 0.26 migration. See `docs/reports/salsa-upgrade-report.md` and the overnight
> hardening report for current status.

**Date:** 2026-02-08
**Branch:** `spike/salsa-stage1-gate`
**Scope:** `crates/trust-hir/src/db/queries/salsa_backend.rs`, `database.rs`, `queries.rs`

---

## Executive Summary

Salsa v0.18 is genuinely integrated and driving incremental computation.
The forum claim that "Salsa is in dependencies but not used" is incorrect.
Five tracked queries (`parse_green`, `file_symbols_query`, `analyze_query`,
`diagnostics_query`, `type_of_query`) use real `#[salsa::tracked]` macros,
and the `Setter` protocol handles incremental invalidation. Ten unit tests
verify cache reuse via `Arc::ptr_eq()`.

However, the **thread-local indirection layer** wrapping Salsa introduces
six bugs ranging from critical to low severity, and three performance
issues. None affect correctness of the incremental computation itself —
they affect the ownership model around it.

---

## Architecture Overview

```
User code
    |
    v
Database  (queries.rs:50-53)
    |  sources: FxHashMap<FileId, Arc<String>>     <-- user-facing source map
    |  salsa_state_id: u64                         <-- key into thread-local
    |
    v  with_state(id, |state| { ... })
    |
Thread-local SALSA_STATES  (salsa_backend.rs:36-38)
    |  RefCell<FxHashMap<u64, SalsaState>>
    |
    v
SalsaState  (salsa_backend.rs:29-34)
    |  db: SalsaDatabase                           <-- real salsa storage
    |  sources: FxHashMap<FileId, SourceInput>      <-- salsa input handles
    |  project_inputs: Option<ProjectInputs>        <-- salsa tracked input
    |
    v
Salsa tracked queries
    parse_green  ->  file_symbols_query  ->  analyze_query
                                          ->  diagnostics_query
                                          ->  type_of_query
```

The indirection exists because `Database` holds a `u64` key rather than
owning `SalsaDatabase` directly. Every query must go through
`with_state()`, which borrows the thread-local `RefCell`, looks up the
`FxHashMap`, and runs a closure.

---

## Confirmed Bugs

### BUG-1: Clone shares mutable Salsa state (CRITICAL)

**Location:** `queries.rs:70-77`

```rust
impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            sources: self.sources.clone(),
            salsa_state_id: self.salsa_state_id,  // same ID
        }
    }
}
```

All clones share the same `salsa_state_id`, meaning they access the same
`SalsaState` in the thread-local map. If clone A calls `set_source_text`:

1. `state.sources[file]` is updated via `Setter` (Salsa sees new text)
2. `self.sources[file]` is updated in clone A's own map

Clone B now has a **split brain**:
- `b.source_text(file)` returns the **old** text (reads `self.sources`)
- `b.file_symbols(file)` returns symbols for the **new** text (reads Salsa)

**Production impact:** The LSP server clones Database on every request via
`with_database()` (`state/mod.rs:392`). Currently safe because clones are
read-only snapshots and mutations go through the original. However, this is
a latent invariant violation — nothing in the type system prevents a clone
from calling `set_source_text()`.

**Fix:** Allocate a fresh `salsa_state_id` in `Clone::clone()` and deep-copy
the `SalsaState`, or embed `SalsaDatabase` directly in `Database`.

---

### BUG-2: No Drop — thread-local state leaks (HIGH)

**Location:** `queries.rs:61-68`, `salsa_backend.rs:36-44`

There is no `impl Drop for Database`. When a `Database` is dropped, its
`salsa_state_id` is lost but the corresponding `SalsaState` entry in
`SALSA_STATES` persists forever.

Each `SalsaState` contains:
- `SalsaDatabase` with `salsa::Storage` (the full memoization cache)
- `FxHashMap<FileId, SourceInput>` (Salsa input handles)
- `Option<ProjectInputs>` (tracked project state)

**Production impact:** Mitigated. The LSP creates one `Database` at startup
that lives for the server session. Test suites create ~70 databases that
leak until the process exits — acceptable for tests but technically wrong.

**Fix:** `impl Drop for Database` that removes the entry from `SALSA_STATES`.

---

### BUG-3: prepare_salsa_project ignores removed files (HIGH)

**Location:** `database.rs:124-138`

```rust
fn prepare_salsa_project(&self, state: &mut salsa_backend::SalsaState) {
    let mut project_changed = false;
    for (&known_file_id, text) in &self.sources {
        if state.sources.contains_key(&known_file_id) {
            continue;                         // only adds, never removes
        }
        // ...
    }
}
```

This method adds new files to `state.sources` but never removes files that
were deleted via `remove_source_text()`. If a file is removed from
`self.sources`, the next call to `prepare_salsa_project` skips it (it's not
in `self.sources`) but doesn't clean it from `state.sources`.

`remove_source_text()` (`database.rs:50-56`) does remove from both maps
explicitly. The real risk is if `prepare_salsa_project` is called
**before** `remove_source_text` completes — which can happen because
`analyze_salsa`, `diagnostics_salsa`, and `type_of_salsa` all call
`prepare_salsa_project` as their first step.

**Production impact:** In the LSP, `remove_source()` is called on document
close/rename (`documents.rs:132,145-146`). If a query races with removal,
it may see stale project state.

**Fix:** Add a reverse sweep in `prepare_salsa_project`:

```rust
state.sources.retain(|file_id, _| self.sources.contains_key(file_id));
```

---

### BUG-4: set_source_text mutation ordering (MEDIUM)

**Location:** `database.rs:166-177`

```rust
fn set_source_text(&mut self, file_id: FileId, text: String) {
    salsa_backend::with_state(self.salsa_state_id, |state| {
        // Mutation 1: Salsa state updated first
        if let Some(source) = state.sources.get(&file_id).copied() {
            source.set_text(&mut state.db).to(text.clone());
        } else {
            let source = salsa_backend::SourceInput::new(&state.db, text.clone());
            state.sources.insert(file_id, source);
        }
        salsa_backend::sync_project_inputs(state);
    });
    self.sources.insert(file_id, Arc::new(text));  // Mutation 2: self.sources
}
```

If `self.sources.insert()` panics (OOM on Arc allocation), Salsa has
already been updated but `self.sources` has not. Subsequent calls to
`source_text()` return the old text while Salsa queries use the new text.

**Fix:** Update `self.sources` first (cheap `Arc::new` + HashMap insert),
then update Salsa state.

---

### BUG-5: invalidate() is dead code (LOW)

**Location:** `database.rs:14-18`

```rust
pub fn invalidate(&mut self, _file_id: FileId) {}
```

This method is never called anywhere in the codebase (confirmed via
grep). It exists as "API compatibility" but communicates a false
capability — callers might assume they can force re-evaluation.

**Fix:** Remove entirely, or mark `#[deprecated]`.

---

### BUG-6: RefCell double-borrow risk on reentrancy (LOW)

**Location:** `salsa_backend.rs:46-52`

```rust
pub(super) fn with_state<R>(state_id: u64, f: impl FnOnce(&mut SalsaState) -> R) -> R {
    SALSA_STATES.with(|states| {
        let mut states = states.borrow_mut();   // runtime borrow
        let state = states.entry(state_id).or_default();
        f(state)
    })
}
```

If the closure `f` ever calls back into `with_state` with the same
`state_id`, the `borrow_mut()` panics at runtime ("already borrowed").

Currently safe because Salsa tracked queries inside `f` don't recursively
call `with_state`. But this is a **fragile invariant** — adding a new
query that calls back through `Database` methods would cause a panic with
no compile-time warning.

**Fix:** Embedding `SalsaDatabase` directly in `Database` eliminates the
`RefCell` entirely, giving compile-time borrow checking.

---

## Performance Issues

### PERF-1: analyze_query re-parses the target file (HIGH)

**Location:** `salsa_backend.rs:103-119`

```rust
fn analyze_query(db, project, file_id) -> Arc<FileAnalysis> {
    let Some(ProjectState { target_input, .. }) =
        collect_project_state(db, project, file_id)   // calls file_symbols_query
    else { ... };                                      //   which calls parse_green

    let parsed = parse(target_input.text(db));         // parses AGAIN
```

`collect_project_state` already triggers `parse_green` (via
`file_symbols_query`), which parses the source and caches the `GreenNode`.
Then line 118 calls `parse()` again on the raw text, creating a second
parse tree from scratch.

`parse_green` returns a `GreenNode`; `parse()` returns a `Parse<Root>`.
The green node from Salsa could be reused to construct the `SyntaxNode`
directly via `SyntaxNode::new_root(green)`, which is exactly what
`file_symbols_query` does at line 98.

**Impact:** Every `analyze` call parses the target file twice. For a 5000-
line file, this is measurable.

**Fix:** Replace `parse(target_input.text(db))` with:
```rust
let green = parse_green(db, target_input).clone();
let root = SyntaxNode::new_root(green);
```

---

### PERF-2: collect_project_used_symbols re-parses all files (HIGH)

**Location:** `salsa_backend.rs:270-308`

```rust
fn collect_project_used_symbols(
    sources: &FxHashMap<FileId, Arc<String>>,
    tables: &FxHashMap<FileId, Arc<SymbolTable>>,
) -> FxHashSet<(FileId, SymbolId)> {
    for (&file_id, source) in sources {
        let parsed = parse(source);           // re-parses EVERY file
```

Called from `analyze_query` at line 146. Every file in the project is
re-parsed from its text to collect used symbols. The parse results are not
reused from `parse_green` because this function takes raw `Arc<String>`
sources instead of `SourceInput` handles.

**Impact:** For N project files, every `analyze` call does N extra parses
beyond what Salsa caches.

**Fix:** Pass `SourceInput` handles and use `parse_green(db, input)` to
get cached green nodes.

---

### PERF-3: no_eq on tracked queries suppresses short-circuiting (MEDIUM)

**Location:** `salsa_backend.rs:95,103,162,171`

All four tracked queries use `#[salsa::tracked(..., no_eq)]`:

```rust
#[salsa::tracked(return_ref, no_eq)]
pub(super) fn file_symbols_query(...) -> Arc<SymbolTable> { ... }
```

The `no_eq` attribute tells Salsa to skip output equality comparison.
Without it, Salsa would compare the old and new outputs; if they're equal,
downstream queries are not re-executed ("early termination"). With `no_eq`,
Salsa always propagates invalidation to downstream queries even if the
output hasn't changed.

This is defensible if `SymbolTable`, `FileAnalysis`, etc. don't implement
`Eq` — and indeed `SymbolTable` only recently got `PartialEq`/`Eq` added
(visible in the diff for `symbols/defs.rs`). But now that `Eq` is derived,
removing `no_eq` from `file_symbols_query` would enable short-circuiting:
if a whitespace-only edit doesn't change the symbol table, `analyze_query`
would skip re-execution.

**Impact:** Unnecessary recomputation when inputs change but outputs don't.

**Fix:** Remove `no_eq` from queries whose return types implement `Eq`.

---

## Test Coverage Gaps

### GAP-1: remove_source_text is untested

No unit test calls `remove_source_text()` directly or verifies that:
- Salsa state is cleaned up after removal
- Subsequent queries return empty/default results for removed files
- Project-wide queries no longer include the removed file

The LSP integration tests cover document lifecycle but don't assert on
Database internals.

### GAP-2: Clone semantics are untested

No test creates a `Database`, clones it, mutates one side, and verifies
the other side's behavior. The split-brain bug (BUG-1) is latent.

### GAP-3: Multi-file removal + re-query is untested

No test adds files, removes some, and verifies that remaining queries
still work correctly with the reduced project set.

---

## Summary

| ID | Severity | Category | Description |
|----|----------|----------|-------------|
| BUG-1 | CRITICAL | Correctness | Clone shares mutable Salsa state |
| BUG-2 | HIGH | Resource | No Drop — thread-local entries leak |
| BUG-3 | HIGH | Correctness | prepare_salsa_project ignores removed files |
| BUG-4 | MEDIUM | Correctness | set_source_text mutation ordering |
| BUG-5 | LOW | Dead code | invalidate() is a no-op, never called |
| BUG-6 | LOW | Safety | RefCell double-borrow risk on reentrancy |
| PERF-1 | HIGH | Performance | analyze_query re-parses target file |
| PERF-2 | HIGH | Performance | collect_project_used_symbols re-parses all files |
| PERF-3 | MEDIUM | Performance | no_eq suppresses Salsa short-circuiting |

---

## Recommended Fix: Embed SalsaDatabase Directly

All six bugs stem from the thread-local indirection. Embedding
`SalsaDatabase` (and associated state) directly as fields of `Database`
eliminates:

- **BUG-1**: Each `Database` owns its Salsa state; clones get independent copies
- **BUG-2**: Salsa state is dropped with `Database` — no leak
- **BUG-3**: No separate `state.sources` to drift — single source of truth
- **BUG-4**: No two-phase mutation — `self` is the only target
- **BUG-6**: No `RefCell` — compile-time borrow checking via `&mut self`

This would change `Database` from:

```rust
pub struct Database {
    sources: FxHashMap<FileId, Arc<String>>,
    salsa_state_id: u64,
}
```

To:

```rust
pub struct Database {
    db: SalsaDatabase,
    sources: FxHashMap<FileId, SourceInput>,
    project_inputs: Option<ProjectInputs>,
}
```

The public API (`SourceDatabase`, `SemanticDatabase` traits) and all 10
tests remain unchanged. The thread-local infrastructure (~25 lines) and
`with_state` pattern are removed entirely.
