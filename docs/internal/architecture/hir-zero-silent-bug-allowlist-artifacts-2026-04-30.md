# HIR Zero-Silent-Bug Allowlist Artifacts - 2026-04-30

This artifact records the explicit compatibility boundaries left after the HIR zero-silent-bug refactor.

## Semantic-Kernel Compatibility Allowlist

Source: `docs/internal/architecture/hir-semantic-kernel-allowlist.toml`

- Maximum entries: 5
- Current entries: 5
- Granularity rule: one entry is one fully qualified function or one Rust file:line range, not a subsystem category.
- Entries:
  - `trust_hir::symbols::table::SymbolTable::resolve_alias_type`
  - `trust_hir::type_check::TypeChecker::resolve_alias_type`
  - `trust_hir::db::queries::collector::SymbolCollector::resolve_type_path_at`
  - `trust_hir::db::symbol_import::SymbolImporter::import_type`
  - `trust_hir::db::diagnostics::oop::resolve_extends_symbol`

## Typed Sentinel And Silent-Discard Allowlists

- Active broad typed-sentinel allowlist: none.
- Active silent `let _ = ...` collision-discard allowlist: none.
- Active inline `Type::Unknown` fallback allowlist: none.

## Validation

- `python3 scripts/hir_zero_silent_bug_doctor.py --self-test`
- `python3 scripts/hir_zero_silent_bug_doctor.py --fail`
- `RUSTUP_TOOLCHAIN=1.95 cargo run -p xtask -- architecture-doctor --full-map`

The full-map command is expected to emit `PASS: FULLMAP-HIRZSB` and fail if the standalone HIR zero-silent-bug doctor reports any finding.
