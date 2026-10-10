# Runtime portability research archive

The canonical product plan is [specification 34](../../specs/34-runtime-portability.md).
Implementation progress, scope boundaries and evidence live in the
[implementation checklist](../../internal/testing/checklists/runtime-portability-implementation-checklist.md).
This directory contains non-normative, dated design evidence:

- [Research notes](research-notes.md): historical input hashes, revision history,
  amendment mapping, dependency candidates, and review dispositions.
- [Registry snapshot](trust-runtime-modernization-inventory-2026-10-09.json): the
  unchanged 9 October 2026 metadata snapshot covering 74 direct Cargo registry
  crates and 30 npm packages, with five Git declarations and 21 manifests.
- [Workflow setup checks](workflow-setup-validation-2026-10-09.md): document consistency
  for the recorded v0.8 specification/checklist bytes only; this does not validate the later
  v0.9/v0.10 scheduling amendments. No runtime or hardware evidence.

Snapshot SHA-256:
`6a088873d343cced6c536442fdd32e63cf0687017c64bf62ad12cea4d1e9f6d0`.
It is a research input, not a resolved upgrade set, advisory audit, or continuously
current inventory. Refresh candidates in the separate M0U modernization scope.
Historical plan/review text does not override the current specification, and
document checks are not runtime or hardware evidence.
