# Claude Review Prompt - Issue #51 Struct Initializers

You are reviewing a pre-implementation diagnosis for trust-platform issue #51. Do not implement code yet. Your job is to validate the diagnosis, risks, test plan, and architecture plan before Codex starts patching.

Read this report first:

- `docs/internal/references/issue-51-struct-initializers/report.md`

Then review the live code/spec/diagram evidence. You must cite exact `file:line` references in your response.

Required files to inspect:

- `docs/specs/03-variables.md`
- `docs/specs/02-data-types.md`
- `docs/specs/10-runtime-semantics.md`
- `docs/diagrams/syntax/syntax-pipeline.puml`
- `docs/diagrams/hir/hir-semantics.puml`
- `docs/diagrams/architecture/system-architecture.puml`
- `docs/diagrams/architecture/runtime-execution.puml`
- `crates/trust-syntax/src/parser/grammar/declarations.rs`
- `crates/trust-syntax/src/parser/grammar/expressions.rs`
- `crates/trust-hir/src/type_check/calls.rs`
- `crates/trust-hir/src/types/defs.rs`
- `crates/trust-runtime/src/harness/lower/expr/lowering.rs`
- `crates/trust-runtime/src/harness/compiler/types.rs`
- `crates/trust-runtime/src/value/defaults.rs`
- `crates/trust-runtime/src/value/types.rs`

Questions to answer:

1. Is the report's root-cause analysis complete, or is there another parser/HIR/runtime layer involved?
2. Is this a simple bug, or an architecture-level initializer/default propagation gap? Explain the blast radius.
3. Why are the silent `STRUCT` field default errors not detected today?
4. Are there other places where parsed initializers are dropped or downgraded silently? Search for similar `_initializer`, unsupported `InitializerList`, or default-construction paths.
5. Is named `STRUCT` initialization required by the repo specs and IEC?
6. Is positional `(2, TRUE)` initialization required by IEC, supported by this repo's specs, or should it be deferred/rejected?
7. Does FB instance initialization `Timer: TON := (PT := T#1s);` share enough grammar/semantics with struct initialization that it must be tested in the same fix?
8. Is `TypeName(field := value)` the correct canonical form to support, and should `(field := value)` be accepted only when an expected struct type exists?
9. Is the proposed implementation order safe: parser tests, HIR typing, runtime lowering, field-default metadata, default construction?
10. Which tests are mandatory before implementation, and where should they live?
11. Which specs, PlantUML diagrams, and architecture checklist entries must change if the implementation proceeds?
12. Is the proposed branch/worktree strategy correct: implement in `/home/johannes/projects/trust-platform-issue-51` on `fix/issue-51-struct-initializers`, leaving the dirty OSCAT checkout untouched?

Output format:

- Start with `Verdict: ready to implement` or `Verdict: not ready`.
- List blocking corrections first.
- Then list non-blocking improvements.
- Then answer the 12 questions above.
- Use exact `file:line` references for every concrete claim.
- Do not edit files.
- Do not run broad test suites unless you explicitly explain why a small command is needed for review.

