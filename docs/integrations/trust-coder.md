This file is a pointer. Authoritative state lives in the separate `trust-coder` repo under `plan/checklists/`. Do not duplicate item status here.

# trust-coder Integration Pointer

Purpose:

- summarize which `trust-platform` features `trust-coder` depends on
- record the currently frozen `TRUST_PLATFORM_COMMIT`
- list open trust-platform-side PRs keyed to `trust-coder` checklist IDs
- point readers at the standalone sibling repo root: `../trust-coder` from the `trust-platform` repo root

Current frozen `TRUST_PLATFORM_COMMIT`:

- `3d51290a4b886014e9200cab891843e5003e73cd`
- Authoritative file in sibling repo: `../trust-coder/TRUST_PLATFORM_COMMIT` from the `trust-platform` repo root
- Local path from this file: `../../../trust-coder/TRUST_PLATFORM_COMMIT`
- This is the substrate commit currently frozen by `trust-coder`; it may intentionally differ from the latest `trust-platform` `main` tip once follow-up pointer-doc or release-hygiene commits land here.

Dependency surface summary:

- parser + semantic diagnostics from `trust-syntax` / `trust-hir`
- machine-readable diagnostics through `trust-dev agent serve` `lsp.diagnostics`, including stable codes, severities, project-relative paths, and zero-based UTF-8 byte spans
- canonical-AST normalization and similarity scoring through `trust-dev agent serve` `lsp.ast_canonicalize` and `lsp.ast_similarity`
- multi-file compile/build/runtime harness execution from the runtime stack, including the stateless `trust-dev agent serve` `harness.execute` fixture wrapper for one-shot POU/system checks
- OOP semantic/runtime grading only where the appendix paths in `§1.8` remain valid at the frozen commit
- benchmark/datagen tooling stays in `trust-coder`; only verifier/compiler/runtime substrate lives here

Open trust-platform-side PRs:

| Checklist ID | PR URL | State | Scope |
|---|---|---|---|
| — | — | — | No open platform-side PRs recorded yet |
