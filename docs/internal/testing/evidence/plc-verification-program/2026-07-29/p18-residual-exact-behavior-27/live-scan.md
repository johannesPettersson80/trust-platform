# Generated Existing-Test Catalog

Generator: `test-catalog-scanner v2`
Source revision: `4c35370499ff2423a22de327a67be444f5c5fdf5`
Generated: `2026-07-29T13:00:00Z`
Platform: `linux-x86_64`
Generated JSON SHA-256: `e6a9ab23451fbf675043bad30adea428e9e46fd13b94e1be0155a18688e53445`
Input SHA-256: `sha256:eae893b3736d3079aec4b570350e099ca6ea3dd31433e9fe7e6550b4cd3dd137`

This is a mechanical source inventory. It does not map tests to claims,
infer expected behavior, or replace hand-owned test catalog metadata.

## Summary

- Records: 4409
- Source files with records: 772
- Ignored records: 23
- Conditional ignore markers: 0
- Visible scan diagnostics: 0
- Scan errors: 0
- Scan warnings: 0

| Source kind | Records |
| --- | ---: |
| `conformance_case` | 21 |
| `fuzz_target` | 8 |
| `gate_script` | 34 |
| `github_workflow_job` | 30 |
| `rust_integration_test` | 1597 |
| `rust_unit_test` | 2001 |
| `structured_text_test` | 257 |
| `vscode_test` | 461 |

## Hand-Owned Intent

The generated JSON explicitly excludes:
- `subject_kind`
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
- `spec_gap_ref`
- `expected_failure_mode`
- `evidence_destination`
- `command`
- `last_reviewed`

## Limitations

- Discovery is static and recognizes only the declaration forms documented for this slice.
- Command hints are navigation aids; non-exact hints are not runnable proof commands.
- Reference candidates are lexical observations and never create proof or invariant mappings.
- Nested Rust integration support files use package-level hints when no Cargo target is evident.
- Dynamic VS Code titles and runtime skips remain visible diagnostics rather than inferred facts.
- Structured Text command hints use project-level substring filters and remain conservative.
- The scripts/*gate* surface means root-level files under scripts whose names contain gate.
- The P2 board scope excludes Rust tests under xtask/**; they are not included in report totals.
- Only the root fuzz/Cargo.toml is scanned; crate-local fuzz workspaces are excluded.
- Reviewed live census counts are an evidence tripwire and require an intentional refresh on drift.
- Hand-owned catalog intent and enforcement remain outside VERIF-P2-001 through VERIF-P2-003.
