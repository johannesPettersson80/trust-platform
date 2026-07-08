# Phase 0 Baseline (VERIF-P0-006, VERIF-P0-007)

Date: 2026-07-08. Base commit `6a7492cae` on branch `ads/client`, dirty
worktree (see caveat). Captured on the local workstation.

## Counts (VERIF-P0-006)

| Surface | Count | Command |
| --- | --- | --- |
| Rust source files (crates/, excl. target) | 1492 | `find crates -name '*.rs' -not -path '*/target/*'` |
| Crate integration test files | 381 | `find crates -path '*/tests/*' -name '*.rs'` |
| Crates with a `tests/` directory | 8 | `ls -d crates/*/tests` |
| Test fixture files | 184 | `find crates -path '*tests/fixtures*' -type f` |
| `#[ignore]` attributes | 86 in 26 files | `rg -c '#\[ignore' --type rust` |
| Conformance case files | 47 | `find conformance/cases -type f` |
| Conformance expected artifacts | 27 | `find conformance/expected -type f` |
| Fuzz targets (workspace `fuzz/`) | 2 | `ls fuzz/fuzz_targets/*.rs` |
| CI workflows | 9 | `ls .github/workflows/*.yml` |
| Spec documents | 23 | `ls docs/specs/*.md` |
| IEC/PLCopen decision/deviation docs | 4 | `IEC_DECISIONS`, `IEC_DEVIATIONS`, `PLCOPEN_DECISIONS`, `PLCOPEN_DEVIATIONS` |
| Gate scripts | 27 | `ls scripts/*gate*` |
| VS Code test files (`.ts`) | 40 | `find editors/vscode/src/test -name '*.ts'` |

Notes:

- `trust-runtime-core` has no `tests/` directory; its tests are in-source
  `#[cfg(test)]` modules. It is the catalog-scanner proof case named in
  `test-taxonomy.md`.
- Fuzz-like gates exist beyond the workspace `fuzz/` directory:
  `runtime_vm_malformed_bytecode_fuzz_gate.sh`, `runtime_comms_fuzz_gate.sh`,
  `salsa_fuzz_gate.sh`. Phase 9 (`VERIF-P9-001`) inventories them.
- Counts were captured on a dirty worktree; they are baseline indicators for
  catalog sizing, not release evidence.

## Dirty-Worktree Caveat (VERIF-P0-007)

The worktree on `ads/client` carries extensive unrelated modified files
(CI workflows, trust-debug, trust-dev, trust-hir, trust-ide, trust-lsp,
conformance docs, CHANGELOG, Cargo metadata, and more). This board's work must
not overwrite them.

Files touched for the verification program so far:

- `docs/internal/testing/checklists/plc-verification-program/` (seven docs)
- `docs/internal/testing/checklists/plc-verification-program-checklist.md`
- `docs/internal/testing/evidence/plc-verification-program/2026-07-08/`
- `.gitignore` (negation lines for the program checklist and evidence trees)
- `AGENTS.md` ("Draft PLC Verification Program" pointer section only)
