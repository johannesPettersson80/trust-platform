---
name: trust-lsp-iec
description: IEC 61131-3 Structured Text workflow for compliance, language behavior, spec updates, and IEC decision/deviation classification in truST. Use whenever editing docs/IEC_DECISIONS.md or docs/IEC_DEVIATIONS.md; reject IEC-deviation entries for IEC-silent, product-specific, platform, runtime, CLI, or truST-only behavior.
---

# truST LSP (IEC 61131-3) Workflow

Use this skill when implementing or reviewing ST language features, validating specs against IEC 61131-3, or updating LSP behavior for compliance.

## Core workflow

1. **Scope**: Identify the feature area (lexing, parsing, types, semantics, standard functions/FBs, OOP, namespaces, runtime-assisted LSP, editor UX).
2. **Map to standard**: Locate the IEC section/table and the matching spec file in `docs/specs/`.
3. **Minimal references**:
   - `docs/specs/README.md` for mapping and scope
   - `docs/internal/standards/IEC 61131-3_2013 Ed3.pdf` when present (authoritative local copy)
   - `docs/internal/standards/iec61131-3.txt` for quick search when present
   - If both IEC files are absent locally and `/home/johannes/Downloads/iec-61131-3.ocr.txt` exists, copy it to `docs/internal/standards/iec61131-3.txt` before IEC proof work.
   - If no IEC source is available locally, state the gap and verify on `trust-builder` or with an explicitly provided standard source before claiming IEC proof.
   - `docs/internal/standards/IEC_ST_FEATURE_MATRIX.md` and `docs/specs/10-runtime-semantics.md` for platform/tooling notes
   - `docs/internal/runtime/trust-runtime-ui-specification.md` for runtime UX details
   - `docs/internal/testing/checklists/lsp.md` and `docs/internal/testing/checklists/lsp-beyond-world-class.md` for feature status
   - `editors/vscode/README.md` for client UX surfaces and test pointers
4. **Cross-check**: Verify behavior against the IEC table/section. Cite the section/table in spec edits.
5. **Classify before recording**:
   - Treat every proposed `docs/IEC_DEVIATIONS.md` edit as blocked until the
     normative conflict is proven. Never use that file as a general behavior,
     compatibility, implementation-note, or product-choice registry, and never
     treat an existing entry as precedent for classification.
   - Add an entry to `docs/IEC_DEVIATIONS.md` only when truST intentionally
     conflicts with, omits, or relaxes a normative IEC requirement.
   - Before editing that file, cite the exact IEC section/table, quote or
     precisely paraphrase the normative requirement, state truST's behavior,
     and explain the concrete conflict. If that conflict cannot be stated, do
     not create a deviation entry.
   - IEC-silent, out-of-scope, implementation-specific, platform/runtime, and
     truST-only API behavior belongs in the relevant product specification. It
     is not an IEC deviation.
   - For example, a runtime API default, CLI compatibility rule, file format,
     protocol version, or tool-specific limit that IEC does not govern belongs
     in its product spec, even when the behavior is intentional and tested.
   - If the IEC text genuinely permits multiple readings, record the selected
     interpretation in `docs/IEC_DECISIONS.md` with the IEC citation.
6. **Coverage updates**: If standard functions are touched, update `docs/specs/coverage/standard-functions-coverage.md`.
7. **Test first**: For every new language/editor feature, bug fix, or intentional behavior change, write the smallest focused syntax/HIR/IDE/LSP/runtime/extension test first. Run it and confirm it reaches the expected behavior assertion and fails because the behavior is missing or wrong; compile, dependency, harness, timeout, and unrelated failures do not count.
8. **Implement minimally**: Change only enough production code to satisfy that behavior, then rerun the same focused test until green before starting another slice. Record the red and green commands/results.
9. **Compile gate**: Extend `crates/trust-runtime/tests/fixtures/complete_program/` to cover any new language feature and keep it compiling; run `cargo test -p trust-runtime --test complete_program`.
10. **Checklist sync**:
   - Update relevant checklist file(s) for touched behavior.
   - Update `docs/internal/masterPlan.md` checkboxes for the active MP item (code checklist first, then detailed test checklist after validation).
11. **Validate**:
   - In trust-platform checkouts on a Raspberry Pi or other slow local host, use the remote builder for broad/full validation first, especially `just test-all`.
   - Ask before starting expensive local commands such as workspace `cargo test`, `cargo test -p trust-runtime ...`, local `just test`, local `just clippy`, or local `just test-all`.
   - Cheap local checks are allowed when narrowly scoped, for example formatting, small touched-crate tests, `cargo test -p xtask`, and static inspections.
   - Run `just fmt` and the full gate on `trust-builder` before declaring completion unless a dedicated staged checklist says otherwise.
12. **Client validation**: When editor UX changes, run VS Code extension tests (`npm test` in `editors/vscode`) and update extension docs as needed.

## Search

Prefer `rg` if installed; otherwise use `grep -R`.

## Where to implement

- `crates/trust-syntax` for lexer/parser
- `crates/trust-hir` for types, type checking, and standard semantics
- `crates/trust-ide` for diagnostics/completions/hover
- `crates/trust-lsp` for LSP wiring
- `crates/trust-runtime`/`crates/trust-debug` when LSP uses runtime/debug control data

## Reference file

See `references/iec.md` for quick IEC lookup and extraction tips.
