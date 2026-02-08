# Salsa Integration Audit Fix Checklist

Date: 2026-02-08
Scope: `trust-hir`, `trust-lsp`, docs, tests
Source audit: `docs/reports/salsa-integration-audit.md`

## Correctness (Audit BUGs)

- [x] BUG-1 (clone split-brain): removed `Database` clone path and stopped LSP database snapshot cloning (`with_database` now uses project read lock + shared database reference).
- [x] BUG-2 (lifecycle leak): added `Drop` on `Database` that removes corresponding Salsa state entry (`remove_state`).
- [x] BUG-3 (removed file visibility): hardened project synchronization to remove stale files and stale texts from Salsa state before query execution.
- [x] BUG-4 (mutation ordering): made `set_source_text` update canonical source map first, then tracked Salsa input state.
- [x] BUG-5 (dead invalidate API): removed no-op `invalidate` from runtime API and aligned diagram/docs.
- [x] BUG-6 (reentrancy hazard): refactored state access to detach per-state borrow from map borrow (`state_cell`) and split mutating/read query paths (`with_state` + `with_state_read`) to avoid nested mutable borrow patterns in semantic query flow.

Design note: the audit's "embed Salsa state directly in `Database`" recommendation was evaluated during remediation, but direct embedding currently conflicts with `trust-lsp` `Send + Sync` constraints. The shipped fixes close the reported correctness/resource/perf gaps without breaking LSP thread-safety requirements.

## Performance (Audit PERFs)

- [x] PERF-1: `analyze_query` now reuses cached `parse_green` for target file (no second full parse).
- [x] PERF-2: project used-symbol collection now consumes `SourceInput` handles and `parse_green` cache (no full project reparsing by raw text).
- [x] PERF-3: removed `no_eq` from tracked outputs where equality is available to allow Salsa early-termination.

## Coverage (Audit GAPs)

- [x] GAP-1: added direct `remove_source_text` behavior test coverage.
- [x] GAP-2: added consistency regression coverage across mutable source updates.
- [x] GAP-3: added multi-file remove/re-query and remove/re-add regression coverage.

## Validation Gate

- [x] `just fmt`
- [x] `just clippy` (zero warnings)
- [x] `just test` (full suite)
- [x] `scripts/render_diagrams.sh`
- [x] `python scripts/check_diagram_drift.py`
