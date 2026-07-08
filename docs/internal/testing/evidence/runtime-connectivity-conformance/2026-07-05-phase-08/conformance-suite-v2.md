# Phase 8 Conformance Suite Growth Evidence

Date: 2026-07-05

Validation copy:
`trust-builder:/home/johannes/projects/trust-platform-rtconn-validation`

Remote gate artifacts:
`trust-builder:/home/johannes/projects/trust-platform-rtconn-validation/gate-artifacts/phase8-conformance/`

## Implementation

Phase 8 keeps the legacy conformance profile stable and adds a versioned v2
profile for expanded proof coverage:

- `conformance/schemas/summary-v2.schema.json`
- `scripts/validate_conformance_summary_schema.py`
- `scripts/render_conformance_summary.py`
- `crates/trust-runtime/src/bin/trust-runtime/conformance/{models,runner,execution,series_values,tests}.rs`

The v1 category contract remains available for legacy-only suites. The v2
profile adds strings, arrays, structs, enums, nested values, OOP dispatch,
references, retain matrix, scheduler, and simulated connector-status comms
determinism categories.

New case directories:

- `conformance/cases/strings/cfm_strings_slice_concat_001`
- `conformance/cases/arrays/cfm_arrays_matrix_update_001`
- `conformance/cases/structs/cfm_structs_initializer_field_update_001`
- `conformance/cases/enums/cfm_enums_variant_assignment_001`
- `conformance/cases/nested_values/cfm_nested_values_matrix_struct_001`
- `conformance/cases/oop_dispatch/cfm_oop_dispatch_interface_super_001`
- `conformance/cases/references/cfm_references_ref_to_deref_write_001`
- `conformance/cases/retain_matrix/cfm_retain_matrix_restart_aliases_001`
- `conformance/cases/scheduler/cfm_scheduler_task_interval_001`
- `conformance/cases/comms_determinism/cfm_comms_determinism_connector_projection_001`

The comms determinism case is manifest-only and runs scripted in-process
connector status transitions. It does not open live sockets.

## Docs And CI

Docs updated:

- `conformance/README.md`
- `conformance/contract.md`
- `conformance/naming.md`
- `conformance/external-run-guide.md`
- `conformance/submissions.md`
- `conformance/known-gaps.md`
- `conformance/failure-taxonomy.md`
- `docs/public/reference/conformance.md`

CI updated in `.github/workflows/ci.yml` to validate v1 and v2 summary schemas,
render a Markdown conformance summary, and upload JSON plus Markdown reports as
workflow artifacts. Generated reports are not committed under
`conformance/reports/`.

The public conformance page was already present in both
`docs/public/reference/index.md` and `mkdocs.yml`; Phase 8 updated that page
instead of adding a new one.

## Local Proof

Commands:

```bash
cargo fmt --all -- --check
git diff --check
cargo run -p trust-runtime --bin trust-runtime -- conformance --suite-root conformance --update-expected --output target/conformance/phase8-update.json
python3 scripts/validate_conformance_summary_schema.py --schema conformance/schemas/summary-v1.schema.json --schema conformance/schemas/summary-v2.schema.json --summary target/conformance/phase8-update.json
python3 scripts/render_conformance_summary.py --input target/conformance/phase8-update.json --output target/conformance/phase8-update.md
target/debug/trust-runtime conformance --suite-root conformance --output target/conformance/phase8-pass-1.json
target/debug/trust-runtime conformance --suite-root conformance --output target/conformance/phase8-pass-2.json
jq 'del(.generated_at_utc) | .results |= map(del(.duration_ms))' target/conformance/phase8-pass-1.json > target/conformance/phase8-pass-1.normalized.json
jq 'del(.generated_at_utc) | .results |= map(del(.duration_ms))' target/conformance/phase8-pass-2.json > target/conformance/phase8-pass-2.normalized.json
diff -u target/conformance/phase8-pass-1.normalized.json target/conformance/phase8-pass-2.normalized.json
cargo test -p trust-runtime --test conformance_cli_command
python3 scripts/check_public_docs_links.py
python3 scripts/check_public_docs_ia.py
```

Results:

- Formatting and whitespace checks passed.
- Expected artifacts were regenerated for the 10 new v2 cases.
- Full conformance suite: 21 total, 21 passed, summary profile
  `trust-conformance-v2`.
- Normalized run-twice diff: clean.
- Summary schema validation: pass for v1 schema, v2 schema, and v2 run output.
- Markdown report rendering: pass.
- `conformance_cli_command`: 1 passed, preserving the legacy v1 CLI behavior
  lock.
- Public docs link and IA checks passed.

## Remote Builder Targeted Proof

Disk preflight before Phase 8 remote gates:

```text
/home/johannes: 112G free
/tmp: 6.7G free
```

Remote command summary:

```bash
cd "$HOME/projects/trust-platform-rtconn-validation"
export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation"
mkdir -p "$CARGO_TARGET_DIR" gate-artifacts/phase8-conformance
cargo test -p trust-runtime --test conformance_cli_command
cargo run -p trust-runtime --bin trust-runtime -- conformance --suite-root conformance --output gate-artifacts/phase8-conformance/pass-1.json > gate-artifacts/phase8-conformance/pass-1.stdout
cargo run -p trust-runtime --bin trust-runtime -- conformance --suite-root conformance --output gate-artifacts/phase8-conformance/pass-2.json > gate-artifacts/phase8-conformance/pass-2.stdout
jq 'del(.generated_at_utc) | .results |= map(del(.duration_ms))' gate-artifacts/phase8-conformance/pass-1.json > gate-artifacts/phase8-conformance/pass-1.normalized.json
jq 'del(.generated_at_utc) | .results |= map(del(.duration_ms))' gate-artifacts/phase8-conformance/pass-2.json > gate-artifacts/phase8-conformance/pass-2.normalized.json
diff -u gate-artifacts/phase8-conformance/pass-1.normalized.json gate-artifacts/phase8-conformance/pass-2.normalized.json
python3 scripts/validate_conformance_summary_schema.py --schema conformance/schemas/summary-v1.schema.json --schema conformance/schemas/summary-v2.schema.json --summary gate-artifacts/phase8-conformance/pass-1.json --summary gate-artifacts/phase8-conformance/pass-2.json
python3 scripts/render_conformance_summary.py --input gate-artifacts/phase8-conformance/pass-1.json --output gate-artifacts/phase8-conformance/pass-1.md
python3 scripts/check_public_docs_links.py
python3 scripts/check_public_docs_ia.py
```

Results:

- `conformance_cli_command`: 1 passed.
- Conformance pass 1: v2, 21 total, 21 passed.
- Conformance pass 2: v2, 21 total, 21 passed.
- Normalized summary diff: clean.
- Summary validation: v1 schema, v2 schema, pass 1, and pass 2 valid.
- Human-readable report rendered to
  `gate-artifacts/phase8-conformance/pass-1.md`.
- Public docs link and IA checks passed.

Remote artifacts:

- `gate-artifacts/phase8-conformance/pass-1.json`
- `gate-artifacts/phase8-conformance/pass-2.json`
- `gate-artifacts/phase8-conformance/pass-1.normalized.json`
- `gate-artifacts/phase8-conformance/pass-2.normalized.json`
- `gate-artifacts/phase8-conformance/pass-1.md`
- `gate-artifacts/phase8-conformance/pass-1.stdout`
- `gate-artifacts/phase8-conformance/pass-2.stdout`

## Common Gate

Remote commands:

```bash
just fmt
just clippy
just test
```

Results:

- `just fmt`: pass.
- `just clippy`: pass.
- `just test`: pass. `cargo-nextest` was absent, so the repo fallback ran
  `cargo test -p trust-runtime --lib` with 838 passed and 16 ignored.

## Full Boundary Gate

Remote command:

```bash
just test-all
```

Result: pass on `trust-builder` against the isolated validation copy. The gate
ran the all-workspace Rust test/doc-test path through
`./scripts/cargo_test_fast_link.sh test --all`; no failures were reported.

## Builder Cleanup

The broad gate rebuilt the shared validation target cache and temporarily left
`/home/johannes` at 55G free. After the user requested builder cleanup, the
generated validation target and regenerated sccache were removed:

```bash
rm -rf "$HOME/.cache/codex-targets/trust-platform-rtconn-validation" "$HOME/.cache/sccache"
```

Final remote disk state:

```text
/home/johannes: 112G free
/tmp: 6.7G free
~/.cache/codex-targets: 4.0K
~/.cache: 1.2G
```

Source validation worktrees and gate artifacts were not deleted.
