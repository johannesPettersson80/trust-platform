# Generated Existing-Test Catalog

Generator: `test-catalog-scanner v1`
Source revision: `d4a768479c8542b8db3874037939b0e8f09e2499`
Generated: `2026-07-09T13:15:00Z`
Platform: `linux-aarch64`
Generated JSON SHA-256: `f5e219e8c9cf5aa6eced682cb6de1ab21b3c23adaae6d40809e83913adde5c18`
Input SHA-256: `sha256:95257ea20be325b9a67f42dcd8b360229d8bbf786fd275a20e2238b535b2e12d`

This is a mechanical source inventory. It does not map tests to claims,
infer expected behavior, or replace hand-owned test catalog metadata.

## Summary

- Records: 3816
- Source files with records: 670
- Ignored records: 85
- Conditional ignore markers: 1
- Visible scan diagnostics: 1
- Scan errors: 0
- Scan warnings: 1

| Source kind | Records |
| --- | ---: |
| `conformance_case` | 21 |
| `fuzz_target` | 2 |
| `gate_script` | 29 |
| `github_workflow_job` | 30 |
| `rust_integration_test` | 1369 |
| `rust_unit_test` | 1652 |
| `structured_text_test` | 257 |
| `vscode_test` | 456 |

## Hand-Owned Intent

The generated JSON explicitly excludes:
- `area`
- `owner`
- `status`
- `test_class`
- `invariants`
- `expected_result`
- `suite_tiers`
- `requires_hardware`
- `requires_network`
- `duration_class`
- `oracle_ref`
- `expected_failure_mode`
- `evidence_destination`

## Limitations

- Discovery is static and recognizes only the declaration forms documented for this slice.
- Command hints are navigation aids; non-exact hints are not runnable proof commands.
- Reference candidates are lexical observations and never create proof or invariant mappings.
- Nested Rust integration support files use package-level hints when no Cargo target is evident.
- Dynamic VS Code titles and runtime skips remain visible diagnostics rather than inferred facts.
- Structured Text command hints use project-level substring filters and remain conservative.
- The scripts/*gate* surface means root-level files under scripts whose names contain gate.
- Hand-owned catalog intent and enforcement remain outside VERIF-P2-001 through VERIF-P2-003.

## Diagnostics

- `editors/vscode/src/test/suite/new-project.test.ts:339` `warning/conditional_runtime_skip`: runtime this.skip() cannot be represented as a declared ignore attribute
